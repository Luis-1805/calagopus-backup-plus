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
    /// Wirksamer Mindestabstand (Sekunden) für diesen Server.
    pub min_interval_seconds: u32,
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

        if routine
            .rename_template
            .as_deref()
            .is_some_and(|t| t.chars().count() > 200)
        {
            return Err(Msg::new("val.rename_template").arg("name", name));
        }

        let cron = schedule::parse(&routine.cron).map_err(|err| {
            Msg::new("val.invalid_cron")
                .arg("name", name)
                .arg("cron", &routine.cron)
                .arg("error", err)
        })?;

        if routine.source == RoutineSource::Create {
            let timezone = server.map_or(chrono_tz::UTC, |s| s.timezone);
            let min_seconds = server.map_or(settings.min_interval_seconds, |s| s.min_interval_seconds);
            let now = chrono::Utc::now().naive_utc();
            if schedule::min_gap(&cron, timezone, now).is_none() {
                return Err(Msg::new("val.no_future_runs").arg("name", name));
            }
            if schedule::too_frequent(&cron, timezone, now, min_seconds) {
                return Err(Msg::new("val.min_interval")
                    .arg("name", name)
                    .arg("interval", schedule::format_interval(min_seconds)));
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

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::{
        models::{Routine, RoutineSource},
        settings::ExtensionSettingsData,
    };

    fn routine(cron: &str) -> Routine {
        Routine {
            uuid: uuid::Uuid::from_u128(1),
            server_uuid: uuid::Uuid::nil(),
            name: "R".into(),
            enabled: true,
            position: 0,
            cron: cron.into(),
            source: RoutineSource::Create,
            source_routine_uuid: None,
            keep_count: 3,
            max_age_hours: 0,
            name_template: "Auto {routine} {date}".into(),
            rename_template: None,
            ignored_files: Vec::new(),
            next_run: None,
            last_run: None,
            last_error: None,
        }
    }

    #[test]
    fn minimum_interval_in_seconds() {
        let mut settings = ExtensionSettingsData::default();

        // Standard: 1 Stunde
        assert!(validate(&[routine("0 0 * * * *")], &settings, None).is_ok());
        assert!(validate(&[routine("0 */30 * * * *")], &settings, None).is_err());

        // Admin erlaubt 1 Minute
        settings.min_interval_seconds = 60;
        assert!(validate(&[routine("0 * * * * *")], &settings, None).is_ok());
        assert!(validate(&[routine("*/30 * * * * *")], &settings, None).is_err());

        // Server-Wert hat Vorrang vor dem globalen
        let server = super::ServerLimits {
            backup_limit: 100,
            timezone: chrono_tz::UTC,
            min_interval_seconds: 10,
        };
        assert!(validate(&[routine("*/10 * * * * *")], &settings, Some(&server)).is_ok());
    }
}
