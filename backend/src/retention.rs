//! Reine Rotations-Logik (GFS – Grandfather/Father/Son), ohne Datenbank- oder Netzwerkzugriff.
//!
//! Jedes neue automatische Backup startet in der Stufe "stündlich". Fällt es aus den
//! `keep_hourly` neuesten stündlichen Backups heraus, wird geprüft, ob eine höhere Stufe
//! "fällig" ist (monatlich vor wöchentlich vor täglich). Ist das der Fall, wird das Backup
//! dorthin befördert statt gelöscht. Danach wird jede Stufe auf ihre Höchstanzahl gekürzt.
//!
//! Mit der Standard-Richtlinie (3 stündlich, 1 wöchentlich, 1 monatlich) liegen so immer
//! genau 5 Backups vor: die drei der letzten drei Stunden, ein wöchentliches und ein
//! monatliches. Alles Ältere wird gelöscht.

use crate::models::{ManagedBackup, Policy, Tier};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

/// Toleranz, damit leicht verspätete Läufe eine Stufe nicht um eine Periode verschieben.
const TOLERANCE_MINUTES: i64 = 15;

#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PlannedAction {
    /// Backup bleibt in seiner Stufe.
    Keep { tier: Tier },
    /// Backup wird in eine höhere Stufe befördert.
    Promote { from: Tier, to: Tier },
    /// Backup wird gelöscht.
    Delete { reason: String },
    /// Backup ist angepinnt (gesperrt) und wird nie automatisch gelöscht.
    Pinned,
    /// Backup läuft noch.
    Running,
    /// Fehlgeschlagenes Backup, das behalten wird (Löschen deaktiviert).
    FailedKept,
}

/// Berechnet für jedes Backup der Gruppe die nächste Aktion.
///
/// `reserve_hourly` reserviert stündliche Plätze für ein Backup, das gleich erstellt wird
/// (nur nötig, wenn das Backup-Limit des Servers sonst überschritten würde).
pub fn plan(
    backups: &[ManagedBackup],
    policy: &Policy,
    reserve_hourly: i32,
) -> Vec<(uuid::Uuid, PlannedAction)> {
    let mut actions: Vec<(uuid::Uuid, PlannedAction)> = Vec::with_capacity(backups.len());

    // Stufen-Listen mit (uuid, created), jeweils neueste zuerst.
    let mut tiers: HashMap<Tier, Vec<(uuid::Uuid, chrono::NaiveDateTime)>> = HashMap::new();
    let mut original_tier: HashMap<uuid::Uuid, Tier> = HashMap::new();

    let mut sorted: Vec<&ManagedBackup> = backups.iter().collect();
    sorted.sort_by(|a, b| b.created.cmp(&a.created).then(b.uuid.cmp(&a.uuid)));

    for backup in &sorted {
        if backup.completed.is_none() {
            actions.push((backup.uuid, PlannedAction::Running));
            continue;
        }

        if !backup.successful {
            if backup.locked {
                actions.push((backup.uuid, PlannedAction::Pinned));
            } else if policy.delete_failed {
                actions.push((
                    backup.uuid,
                    PlannedAction::Delete {
                        reason: "Backup ist fehlgeschlagen".into(),
                    },
                ));
            } else {
                actions.push((backup.uuid, PlannedAction::FailedKept));
            }
            continue;
        }

        if backup.locked {
            actions.push((backup.uuid, PlannedAction::Pinned));
            continue;
        }

        original_tier.insert(backup.uuid, backup.tier);
        tiers
            .entry(backup.tier)
            .or_default()
            .push((backup.uuid, backup.created));
    }

    // 1. Stündliche Stufe: die neuesten N behalten, der Rest ist Kandidat für eine Beförderung.
    let keep_hourly = (policy.keep(Tier::Hourly) - reserve_hourly.max(0)).max(0) as usize;
    let hourly = tiers.remove(&Tier::Hourly).unwrap_or_default();
    let (kept_hourly, expiring) = if hourly.len() > keep_hourly {
        let (a, b) = hourly.split_at(keep_hourly);
        (a.to_vec(), b.to_vec())
    } else {
        (hourly, Vec::new())
    };
    tiers.insert(Tier::Hourly, kept_hourly);

    let mut promoted: HashMap<uuid::Uuid, Tier> = HashMap::new();
    let mut deleted: HashMap<uuid::Uuid, String> = HashMap::new();
    let tolerance = chrono::Duration::minutes(TOLERANCE_MINUTES);

    // Neueste zuerst: das frischeste auslaufende Backup wird bevorzugt befördert.
    for (uuid, created) in expiring {
        let target = [Tier::Monthly, Tier::Weekly, Tier::Daily]
            .into_iter()
            .find(|tier| {
                if policy.keep(*tier) == 0 {
                    return false;
                }

                match tiers.get(tier).and_then(|list| list.iter().map(|(_, c)| *c).max()) {
                    None => true,
                    Some(newest) => newest + tier.period() <= created + tolerance,
                }
            });

        match target {
            Some(tier) => {
                let list = tiers.entry(tier).or_default();
                list.push((uuid, created));
                list.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
                promoted.insert(uuid, tier);
            }
            None => {
                deleted.insert(
                    uuid,
                    format!(
                        "älter als die {} neuesten stündlichen Backups",
                        policy.keep(Tier::Hourly)
                    ),
                );
            }
        }
    }

    // 2. Jede höhere Stufe auf ihre Höchstanzahl kürzen.
    for tier in [Tier::Daily, Tier::Weekly, Tier::Monthly] {
        let keep = policy.keep(tier) as usize;
        if let Some(list) = tiers.get_mut(&tier) {
            list.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
            if list.len() > keep {
                for (uuid, _) in list.split_off(keep) {
                    promoted.remove(&uuid);
                    deleted.insert(
                        uuid,
                        if keep == 0 {
                            format!("Stufe \"{}\" ist deaktiviert", tier.label())
                        } else {
                            format!(
                                "ersetzt durch ein neueres {} Backup",
                                tier.label().to_lowercase()
                            )
                        },
                    );
                }
            }
        }
    }

    // 3. Ergebnis zusammensetzen.
    for (tier, list) in &tiers {
        for (uuid, _) in list {
            let from = original_tier.get(uuid).copied().unwrap_or(Tier::Hourly);
            match promoted.get(uuid) {
                Some(to) if *to != from => actions.push((
                    *uuid,
                    PlannedAction::Promote {
                        from,
                        to: *to,
                    },
                )),
                _ => actions.push((*uuid, PlannedAction::Keep { tier: *tier })),
            }
        }
    }
    for (uuid, reason) in deleted {
        actions.push((uuid, PlannedAction::Delete { reason }));
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backup(hours_ago: i64, tier: Tier) -> ManagedBackup {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let created = now - chrono::Duration::hours(hours_ago);
        ManagedBackup {
            uuid: uuid::Uuid::from_u128(
                NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as u128,
            ),
            name: format!("b-{hours_ago}"),
            tier,
            successful: true,
            locked: false,
            bytes: 1,
            completed: Some(created + chrono::Duration::minutes(2)),
            created,
        }
    }

    fn count(actions: &[(uuid::Uuid, PlannedAction)], pred: impl Fn(&PlannedAction) -> bool) -> usize {
        actions.iter().filter(|(_, a)| pred(a)).count()
    }

    #[test]
    fn first_expiring_backups_get_promoted_to_monthly_then_weekly() {
        let policy = Policy::default_for(uuid::Uuid::nil());
        let backups: Vec<_> = (0..5).map(|h| backup(h, Tier::Hourly)).collect();
        let actions = plan(&backups, &policy, 0);

        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Keep { tier: Tier::Hourly })), 3);
        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Promote { to: Tier::Monthly, .. })), 1);
        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Promote { to: Tier::Weekly, .. })), 1);
        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Delete { .. })), 0);
    }

    #[test]
    fn steady_state_keeps_exactly_five() {
        let policy = Policy::default_for(uuid::Uuid::nil());
        let mut backups: Vec<_> = (0..4).map(|h| backup(h, Tier::Hourly)).collect();
        backups.push(backup(48, Tier::Weekly));
        backups.push(backup(24 * 10, Tier::Monthly));
        let actions = plan(&backups, &policy, 0);

        let kept = count(&actions, |a| matches!(a, PlannedAction::Keep { .. } | PlannedAction::Promote { .. }));
        assert_eq!(kept, 5);
        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Delete { .. })), 1);
    }

    #[test]
    fn weekly_rotates_after_seven_days() {
        let policy = Policy::default_for(uuid::Uuid::nil());
        let mut backups: Vec<_> = (0..4).map(|h| backup(h, Tier::Hourly)).collect();
        let old_weekly = backup(24 * 7 + 5, Tier::Weekly);
        let old_weekly_uuid = old_weekly.uuid;
        backups.push(old_weekly);
        backups.push(backup(24 * 10, Tier::Monthly));
        let actions = plan(&backups, &policy, 0);

        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Promote { to: Tier::Weekly, .. })), 1);
        assert!(actions.iter().any(|(u, a)| *u == old_weekly_uuid && matches!(a, PlannedAction::Delete { .. })));
    }

    #[test]
    fn pinned_and_running_are_never_deleted() {
        let policy = Policy::default_for(uuid::Uuid::nil());
        let mut backups: Vec<_> = (0..10).map(|h| backup(h, Tier::Hourly)).collect();
        backups[9].locked = true;
        backups[8].completed = None;
        let actions = plan(&backups, &policy, 0);

        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Pinned)), 1);
        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Running)), 1);
    }

    #[test]
    fn reserve_frees_one_hourly_slot() {
        let mut policy = Policy::default_for(uuid::Uuid::nil());
        policy.keep_weekly = 0;
        policy.keep_monthly = 0;
        let backups: Vec<_> = (0..3).map(|h| backup(h, Tier::Hourly)).collect();
        let actions = plan(&backups, &policy, 1);

        assert_eq!(count(&actions, |a| matches!(a, PlannedAction::Delete { .. })), 1);
    }
}
