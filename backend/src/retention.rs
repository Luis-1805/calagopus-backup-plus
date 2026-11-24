//! Reine Rotations-Logik, ohne Datenbank- oder Netzwerkzugriff.
//!
//! Jede Routine behält höchstens `keep` Backups (und optional keine, die älter als
//! `max_age` sind). Fällt ein Backup aus einer Routine heraus, wird geprüft, ob eine
//! Routine mit der Quelle "Übernehmen von <dieser Routine>" gerade fällig ist. Ist das der
//! Fall, wechselt das Backup dorthin, statt gelöscht zu werden. So lassen sich z. B.
//! "3 stündliche + 1 wöchentliches + 1 monatliches" Backups abbilden, ohne zusätzliche
//! Backups zu erzeugen. Ketten sind möglich (stündlich → täglich → wöchentlich …).

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;

/// Was die Planung über eine Routine wissen muss.
#[derive(Clone, Debug)]
pub struct RoutineSpec {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub enabled: bool,
    pub position: i32,
    pub keep: i32,
    pub max_age: Option<chrono::Duration>,
    /// Quelle, falls die Routine Backups aus einer anderen Routine übernimmt.
    pub promote_from: Option<uuid::Uuid>,
    /// Ob die Routine laut Zeitplan gerade an der Reihe ist.
    pub due: bool,
}

/// Was die Planung über ein Backup wissen muss.
#[derive(Clone, Debug)]
pub struct BackupSpec {
    pub uuid: uuid::Uuid,
    pub routine: Option<uuid::Uuid>,
    pub created: chrono::NaiveDateTime,
    pub completed: bool,
    pub successful: bool,
    pub locked: bool,
}

#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PlannedAction {
    /// Backup bleibt in seiner Routine.
    Keep { routine: uuid::Uuid },
    /// Backup wechselt in eine andere Routine.
    Move { from: uuid::Uuid, to: uuid::Uuid },
    /// Backup wird gelöscht.
    Delete { reason: String },
    /// Backup ist angepinnt (gesperrt) und wird nie automatisch gelöscht.
    Pinned,
    /// Backup läuft noch.
    Running,
    /// Fehlgeschlagenes Backup, das behalten wird (Löschen deaktiviert).
    FailedKept,
    /// Backup gehört zu keiner (aktiven) Routine und wird nicht angefasst.
    Unassigned,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub actions: Vec<(uuid::Uuid, PlannedAction)>,
    /// Routinen, die in diesem Durchlauf ein Backup übernommen haben.
    pub received: Vec<uuid::Uuid>,
}

impl Plan {
    pub fn action_for(&self, uuid: uuid::Uuid) -> Option<&PlannedAction> {
        self.actions.iter().find(|(u, _)| *u == uuid).map(|(_, a)| a)
    }
}

/// Sortiert Routinen so, dass Quellen vor den Routinen kommen, die von ihnen übernehmen.
fn processing_order(routines: &[RoutineSpec]) -> Vec<&RoutineSpec> {
    let by_uuid: HashMap<uuid::Uuid, &RoutineSpec> = routines.iter().map(|r| (r.uuid, r)).collect();

    let depth = |routine: &RoutineSpec| {
        let mut depth = 0;
        let mut current = routine.promote_from;
        while let Some(source) = current {
            depth += 1;
            if depth > routines.len() {
                break; // Zyklus – wird bei der Validierung verhindert
            }
            current = by_uuid.get(&source).and_then(|r| r.promote_from);
        }
        depth
    };

    let mut ordered: Vec<&RoutineSpec> = routines.iter().collect();
    ordered.sort_by_key(|r| (depth(r), r.position));
    ordered
}

/// Berechnet für jedes Backup die nächste Aktion.
///
/// `reserve` reserviert in der genannten Routine einen Platz für ein Backup, das gleich
/// erstellt wird (nur nötig, wenn sonst das Backup-Limit des Servers überschritten würde).
pub fn plan(
    backups: &[BackupSpec],
    routines: &[RoutineSpec],
    delete_failed: bool,
    now: chrono::NaiveDateTime,
    reserve: Option<uuid::Uuid>,
) -> Plan {
    let mut plan = Plan::default();
    let active: HashMap<uuid::Uuid, &RoutineSpec> = routines
        .iter()
        .filter(|r| r.enabled)
        .map(|r| (r.uuid, r))
        .collect();

    // Aktive Backups je Routine, jeweils (uuid, created, ursprüngliche Routine).
    let mut lists: HashMap<uuid::Uuid, Vec<(uuid::Uuid, chrono::NaiveDateTime, uuid::Uuid)>> =
        HashMap::new();

    for backup in backups {
        if !backup.completed {
            plan.actions.push((backup.uuid, PlannedAction::Running));
            continue;
        }

        if backup.locked {
            plan.actions.push((backup.uuid, PlannedAction::Pinned));
            continue;
        }

        if !backup.successful {
            plan.actions.push((
                backup.uuid,
                if delete_failed {
                    PlannedAction::Delete {
                        reason: "Backup ist fehlgeschlagen".into(),
                    }
                } else {
                    PlannedAction::FailedKept
                },
            ));
            continue;
        }

        match backup.routine.filter(|r| active.contains_key(r)) {
            Some(routine) => lists
                .entry(routine)
                .or_default()
                .push((backup.uuid, backup.created, routine)),
            None => plan.actions.push((backup.uuid, PlannedAction::Unassigned)),
        }
    }

    let mut received: HashSet<uuid::Uuid> = HashSet::new();
    let ordered: Vec<&RoutineSpec> = processing_order(routines)
        .into_iter()
        .filter(|r| r.enabled)
        .collect();

    for routine in &ordered {
        let mut list = lists.remove(&routine.uuid).unwrap_or_default();
        list.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

        let keep = (routine.keep - i32::from(reserve == Some(routine.uuid))).max(0) as usize;

        let mut kept = Vec::new();
        let mut expiring = Vec::new();
        for (index, item) in list.into_iter().enumerate() {
            let too_old = routine.max_age.is_some_and(|max| now - item.1 > max);
            if index < keep && !too_old {
                kept.push(item);
            } else {
                expiring.push((item, too_old));
            }
        }

        for item in kept {
            plan.actions.push((
                item.0,
                if item.2 == routine.uuid {
                    PlannedAction::Keep {
                        routine: routine.uuid,
                    }
                } else {
                    PlannedAction::Move {
                        from: item.2,
                        to: routine.uuid,
                    }
                },
            ));
        }

        // Neueste zuerst: das frischeste auslaufende Backup wird bevorzugt übernommen.
        for ((uuid, created, origin), too_old) in expiring {
            let target = ordered.iter().find(|candidate| {
                candidate.promote_from == Some(routine.uuid)
                    && candidate.due
                    && !received.contains(&candidate.uuid)
            });

            match target {
                Some(target) => {
                    received.insert(target.uuid);
                    lists
                        .entry(target.uuid)
                        .or_default()
                        .push((uuid, created, origin));
                }
                None => plan.actions.push((
                    uuid,
                    PlannedAction::Delete {
                        reason: if too_old {
                            format!("älter als das Höchstalter der Routine \"{}\"", routine.name)
                        } else {
                            format!(
                                "älter als die {} neuesten Backups der Routine \"{}\"",
                                routine.keep, routine.name
                            )
                        },
                    },
                )),
            }
        }
    }

    // Übrig gebliebene Einträge gehören zu Routinen, die nicht verarbeitet wurden.
    for (_, list) in lists {
        for (uuid, _, _) in list {
            plan.actions.push((uuid, PlannedAction::Unassigned));
        }
    }

    plan.received = received.into_iter().collect();
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> uuid::Uuid {
        uuid::Uuid::from_u128(n)
    }

    fn base() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    fn backup(n: u128, hours_ago: i64, routine: uuid::Uuid) -> BackupSpec {
        BackupSpec {
            uuid: id(n),
            routine: Some(routine),
            created: base() - chrono::Duration::hours(hours_ago),
            completed: true,
            successful: true,
            locked: false,
        }
    }

    /// Die Vorlage "3 stündlich + 1 wöchentlich + 1 monatlich".
    fn gfs(weekly_due: bool, monthly_due: bool) -> Vec<RoutineSpec> {
        vec![
            RoutineSpec {
                uuid: id(1),
                name: "Stündlich".into(),
                enabled: true,
                position: 0,
                keep: 3,
                max_age: None,
                promote_from: None,
                due: false,
            },
            RoutineSpec {
                uuid: id(2),
                name: "Wöchentlich".into(),
                enabled: true,
                position: 1,
                keep: 1,
                max_age: None,
                promote_from: Some(id(1)),
                due: weekly_due,
            },
            RoutineSpec {
                uuid: id(3),
                name: "Monatlich".into(),
                enabled: true,
                position: 2,
                keep: 1,
                max_age: None,
                promote_from: Some(id(1)),
                due: monthly_due,
            },
        ]
    }

    fn count(plan: &Plan, pred: impl Fn(&PlannedAction) -> bool) -> usize {
        plan.actions.iter().filter(|(_, a)| pred(a)).count()
    }

    #[test]
    fn expiring_hourly_backups_fill_due_routines() {
        let backups: Vec<_> = (0..5).map(|h| backup(100 + h as u128, h, id(1))).collect();
        let plan = plan(&backups, &gfs(true, true), true, base(), None);

        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Keep { .. })), 3);
        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Move { to, .. } if *to == id(2))), 1);
        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Move { to, .. } if *to == id(3))), 1);
        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Delete { .. })), 0);
        assert_eq!(plan.received.len(), 2);
    }

    #[test]
    fn not_due_means_delete() {
        let mut backups: Vec<_> = (0..4).map(|h| backup(100 + h as u128, h, id(1))).collect();
        backups.push(backup(200, 48, id(2)));
        backups.push(backup(300, 240, id(3)));
        let plan = plan(&backups, &gfs(false, false), true, base(), None);

        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Keep { .. })), 5);
        assert_eq!(plan.action_for(id(103)), Some(&PlannedAction::Delete {
            reason: "älter als die 3 neuesten Backups der Routine \"Stündlich\"".into()
        }));
    }

    #[test]
    fn due_weekly_replaces_old_weekly() {
        let mut backups: Vec<_> = (0..4).map(|h| backup(100 + h as u128, h, id(1))).collect();
        backups.push(backup(200, 24 * 7 + 3, id(2)));
        backups.push(backup(300, 240, id(3)));
        let plan = plan(&backups, &gfs(true, false), true, base(), None);

        assert_eq!(plan.action_for(id(103)), Some(&PlannedAction::Move { from: id(1), to: id(2) }));
        assert!(matches!(plan.action_for(id(200)), Some(PlannedAction::Delete { .. })));
        assert_eq!(plan.received, vec![id(2)]);
    }

    #[test]
    fn chains_promote_further() {
        // stündlich(2) → täglich(1) → wöchentlich(1)
        let routines = vec![
            RoutineSpec { uuid: id(1), name: "S".into(), enabled: true, position: 0, keep: 2, max_age: None, promote_from: None, due: false },
            RoutineSpec { uuid: id(2), name: "T".into(), enabled: true, position: 1, keep: 1, max_age: None, promote_from: Some(id(1)), due: true },
            RoutineSpec { uuid: id(3), name: "W".into(), enabled: true, position: 2, keep: 1, max_age: None, promote_from: Some(id(2)), due: true },
        ];
        let backups = vec![
            backup(10, 0, id(1)),
            backup(11, 1, id(1)),
            backup(12, 2, id(1)),
            backup(20, 30, id(2)),
        ];
        let plan = plan(&backups, &routines, true, base(), None);

        assert_eq!(plan.action_for(id(12)), Some(&PlannedAction::Move { from: id(1), to: id(2) }));
        assert_eq!(plan.action_for(id(20)), Some(&PlannedAction::Move { from: id(2), to: id(3) }));
        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Delete { .. })), 0);
    }

    #[test]
    fn max_age_deletes_old_backups() {
        let routines = vec![RoutineSpec {
            uuid: id(1),
            name: "Täglich".into(),
            enabled: true,
            position: 0,
            keep: 10,
            max_age: Some(chrono::Duration::days(2)),
            promote_from: None,
            due: false,
        }];
        let backups = vec![backup(1, 24, id(1)), backup(2, 72, id(1))];
        let plan = plan(&backups, &routines, true, base(), None);

        assert!(matches!(plan.action_for(id(1)), Some(PlannedAction::Keep { .. })));
        assert!(matches!(plan.action_for(id(2)), Some(PlannedAction::Delete { .. })));
    }

    #[test]
    fn pinned_running_failed_and_unassigned() {
        let mut backups: Vec<_> = (0..6).map(|h| backup(100 + h as u128, h, id(1))).collect();
        backups[5].locked = true;
        backups[4].completed = false;
        backups[3].successful = false;
        backups.push(BackupSpec { routine: None, ..backup(900, 50, id(1)) });
        backups.push(backup(901, 50, id(99))); // gelöschte Routine
        let plan = plan(&backups, &gfs(false, false), true, base(), None);

        assert_eq!(plan.action_for(id(105)), Some(&PlannedAction::Pinned));
        assert_eq!(plan.action_for(id(104)), Some(&PlannedAction::Running));
        assert!(matches!(plan.action_for(id(103)), Some(PlannedAction::Delete { .. })));
        assert_eq!(plan.action_for(id(900)), Some(&PlannedAction::Unassigned));
        assert_eq!(plan.action_for(id(901)), Some(&PlannedAction::Unassigned));
    }

    #[test]
    fn disabled_routine_keeps_its_backups() {
        let mut routines = gfs(false, false);
        routines[0].enabled = false;
        let backups: Vec<_> = (0..6).map(|h| backup(100 + h as u128, h, id(1))).collect();
        let plan = plan(&backups, &routines, true, base(), None);

        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Unassigned)), 6);
    }

    #[test]
    fn reserve_frees_one_slot() {
        let backups: Vec<_> = (0..3).map(|h| backup(100 + h as u128, h, id(1))).collect();
        let plan = plan(&backups, &gfs(false, false), true, base(), Some(id(1)));

        assert_eq!(count(&plan, |a| matches!(a, PlannedAction::Delete { .. })), 1);
    }
}
