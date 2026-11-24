//! Simulation: Vorlagen über viele Tage mit echten Cron-Zeitplänen durchspielen.

use crate::{
    models::{Routine, RoutineSource},
    retention::{self, BackupSpec, PlannedAction, RoutineSpec},
    schedule, templates,
};

struct Sim {
    routines: Vec<Routine>,
    backups: Vec<BackupSpec>,
    tz: chrono_tz::Tz,
    next_id: u128,
}

impl Sim {
    fn new(template_id: &str, tz: chrono_tz::Tz) -> Self {
        let template = templates::builtin()
            .into_iter()
            .find(|t| t.id == template_id)
            .unwrap();
        Self {
            routines: templates::to_routines(&template, uuid::Uuid::nil()).unwrap(),
            backups: Vec::new(),
            tz,
            next_id: 1,
        }
    }

    fn routine(&self, name: &str) -> uuid::Uuid {
        self.routines.iter().find(|r| r.name == name).unwrap().uuid
    }

    fn count(&self, name: &str) -> usize {
        let uuid = self.routine(name);
        self.backups.iter().filter(|b| b.routine == Some(uuid)).count()
    }

    fn newest_age(&self, name: &str, now: chrono::NaiveDateTime) -> chrono::Duration {
        let uuid = self.routine(name);
        self.backups
            .iter()
            .filter(|b| b.routine == Some(uuid))
            .map(|b| now - b.created)
            .min()
            .unwrap()
    }

    /// Ein Minuten-Tick: fällige erstellende Routinen anstoßen, danach rotieren.
    fn tick(&mut self, now: chrono::NaiveDateTime) {
        // Backups sind nach 2 Minuten fertig.
        for backup in &mut self.backups {
            if now - backup.created >= chrono::Duration::minutes(2) {
                backup.completed = true;
            }
        }

        // Erstellen (wie engine::run_server mit Trigger::Scheduled).
        if let Some(routine) = self
            .routines
            .iter_mut()
            .find(|r| r.source == RoutineSource::Create && r.is_due(now))
        {
            self.backups.push(BackupSpec {
                uuid: uuid::Uuid::from_u128(self.next_id),
                routine: Some(routine.uuid),
                created: now,
                completed: false,
                successful: true,
                locked: false,
            });
            self.next_id += 1;
            let cron = schedule::parse(&routine.cron).unwrap();
            routine.next_run = schedule::next_after(&cron, self.tz, now);
        }

        // Rotation (wie engine::apply_retention).
        let specs: Vec<RoutineSpec> = self
            .routines
            .iter()
            .map(|r| RoutineSpec {
                uuid: r.uuid,
                name: r.name.clone(),
                enabled: r.enabled,
                position: r.position,
                keep: r.keep_count,
                max_age: (r.max_age_hours > 0)
                    .then(|| chrono::Duration::hours(r.max_age_hours as i64)),
                promote_from: (r.source == RoutineSource::Promote)
                    .then_some(r.source_routine_uuid)
                    .flatten(),
                due: r.source == RoutineSource::Promote && r.is_due(now),
            })
            .collect();
        let plan = retention::plan(&self.backups, &specs, true, now, None);

        for (uuid, action) in &plan.actions {
            match action {
                PlannedAction::Move { to, .. } => {
                    self.backups.iter_mut().find(|b| b.uuid == *uuid).unwrap().routine = Some(*to)
                }
                PlannedAction::Delete { .. } => self.backups.retain(|b| b.uuid != *uuid),
                _ => {}
            }
        }
        for routine in &mut self.routines {
            if plan.received.contains(&routine.uuid) {
                let cron = schedule::parse(&routine.cron).unwrap();
                routine.next_run = schedule::next_after(&cron, self.tz, now);
            }
        }
    }
}

fn start() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
}

/// Dein Setup: immer 3 Backups der letzten Stunden + 1 wöchentliches + 1 monatliches.
#[test]
fn gfs_compact_keeps_exactly_five_for_90_days() {
    let mut sim = Sim::new(templates::DEFAULT_TEMPLATE_ID, chrono_tz::Europe::Berlin);
    let mut max = 0;

    for minute in 0..(60 * 24 * 90) {
        let now = start() + chrono::Duration::minutes(minute);
        sim.tick(now);
        max = max.max(sim.backups.len());

        // Ab der 6. Stunde: stabiler Zustand.
        if minute >= 6 * 60 {
            assert_eq!(sim.count("Stündlich"), if sim.backups.iter().any(|b| !b.completed) { 4 } else { 3 }, "minute {minute}");
            assert_eq!(sim.count("Wöchentlich"), 1, "minute {minute}");
            assert_eq!(sim.count("Monatlich"), 1, "minute {minute}");
            assert!(sim.newest_age("Stündlich", now) <= chrono::Duration::hours(1));
            assert!(sim.newest_age("Wöchentlich", now) <= chrono::Duration::days(7) + chrono::Duration::hours(4), "minute {minute}");
            assert!(sim.newest_age("Monatlich", now) <= chrono::Duration::days(31) + chrono::Duration::hours(4), "minute {minute}");
        }
    }

    // Höchstens kurzzeitig ein laufendes Backup zusätzlich.
    assert_eq!(max, 6);
}

/// Kettenvorlage: 24 stündlich → 7 täglich → 4 wöchentlich → 6 monatlich.
#[test]
fn gfs_extended_settles_at_41() {
    let mut sim = Sim::new("gfs-erweitert", chrono_tz::UTC);

    for minute in 0..(60 * 24 * 220) {
        sim.tick(start() + chrono::Duration::minutes(minute));
    }

    assert_eq!(sim.count("Täglich"), 7);
    assert_eq!(sim.count("Wöchentlich"), 4);
    assert_eq!(sim.count("Monatlich"), 6);
    assert!(sim.count("Stündlich") >= 24);
}
