//! Prüft eine Routinen-Konfiguration auf Gültigkeit.

use crate::{
    i18n::Msg,
    models::{Routine, RoutineSource},
    schedule,
    settings::ExtensionSettingsData,
};
use std::collections::{HashMap, HashSet};

/// Optionale Server-Grenzen (fehlen beim Prüfen von Vorlagen im Admin-Bereich).
pub struct ServerLimits {
    pub backup_limit: i32,
    pub timezone: chrono_tz::Tz,
}

pub fn validate(
    routines: &[Routine],
    settings: &ExtensionSettingsData,
    server: Option<&ServerLimits>,
) -> Result<(), Msg> {
    if routines.len() as u32 > settings.max_routines {
        return Err(Msg::new("val.too_many_routines").arg("max", settings.max_routines));
    }

    let mut names = HashSet::new();
    for routine in routines {
        let name = routine.name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(Msg::new("val.name_required"));
        }
        if !names.insert(name.to_lowercase()) {
            return Err(Msg::new("val.name_duplicate").arg("name", name));
        }
        if !(1..=500).contains(&routine.keep_count) {
            return Err(Msg::new("val.keep_range").arg("name", name));
        }
        if !(0..=87_600).contains(&routine.max_age_hours) {
            return Err(Msg::new("val.max_age_range").arg("name", name));
        }
        if routine.name_template.trim().is_empty() || routine.name_template.chars().count() > 200 {
            return Err(Msg::new("val.name_template").arg("name", name));
        }

        let cron = schedule::parse(&routine.cron).map_err(|err| {
            Msg::new("val.invalid_cron")
                .arg("name", name)
                .arg("cron", &routine.cron)
                .arg("error", err)
        })?;

        if routine.source == RoutineSource::Create {
            let timezone = server.map_or(chrono_tz::UTC, |s| s.timezone);
            let gap = schedule::min_gap(&cron, timezone, chrono::Utc::now().naive_utc())
                .ok_or_else(|| Msg::new("val.no_future_runs").arg("name", name))?;
            // 1 Minute Toleranz für Sommerzeit-Umstellungen
            let min = chrono::Duration::hours(settings.min_interval_hours as i64)
                - chrono::Duration::minutes(1);
            if gap < min {
                return Err(Msg::new("val.min_interval")
                    .arg("name", name)
                    .arg("hours", settings.min_interval_hours));
            }
        }
    }

    // Quellen prüfen: vorhanden, kein Zyklus, Kette endet bei einer erstellenden Routine.
    let by_uuid: HashMap<uuid::Uuid, &Routine> = routines.iter().map(|r| (r.uuid, r)).collect();
    for routine in routines {
        if routine.source != RoutineSource::Promote {
            continue;
        }

        let mut seen = HashSet::from([routine.uuid]);
        let mut current = routine;
        loop {
            let Some(source) = current
                .source_routine_uuid
                .and_then(|uuid| by_uuid.get(&uuid).copied())
            else {
                return Err(Msg::new("val.source_missing").arg("name", &routine.name));
            };
            if !seen.insert(source.uuid) {
                return Err(Msg::new("val.source_cycle").arg("name", &routine.name));
            }
            if source.source == RoutineSource::Create {
                break;
            }
            current = source;
        }
    }

    let keep_total: i32 = routines.iter().map(|r| r.keep_count).sum();
    if keep_total as u32 > settings.max_keep_total {
        return Err(Msg::new("val.keep_total")
            .arg("max", settings.max_keep_total)
            .arg("total", keep_total));
    }
    if let Some(server) = server
        && keep_total > server.backup_limit
    {
        return Err(Msg::new("val.backup_limit")
            .arg("limit", server.backup_limit)
            .arg("total", keep_total));
    }

    Ok(())
}
