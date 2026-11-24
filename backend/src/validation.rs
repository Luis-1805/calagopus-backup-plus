//! Prüft eine Routinen-Konfiguration auf Gültigkeit.

use crate::{
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
) -> Result<(), String> {
    if routines.len() as u32 > settings.max_routines {
        return Err(format!(
            "Es sind höchstens {} Routinen pro Server erlaubt.",
            settings.max_routines
        ));
    }

    let mut names = HashSet::new();
    for routine in routines {
        let name = routine.name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err("Jede Routine braucht einen Namen (1–64 Zeichen).".into());
        }
        if !names.insert(name.to_lowercase()) {
            return Err(format!("Der Routinen-Name \"{name}\" ist doppelt vergeben."));
        }
        if !(1..=500).contains(&routine.keep_count) {
            return Err(format!("\"{name}\": Anzahl behaltener Backups muss zwischen 1 und 500 liegen."));
        }
        if !(0..=87_600).contains(&routine.max_age_hours) {
            return Err(format!("\"{name}\": Höchstalter muss zwischen 0 und 87600 Stunden liegen."));
        }
        if routine.name_template.trim().is_empty() || routine.name_template.chars().count() > 200 {
            return Err(format!("\"{name}\": Namensvorlage muss 1–200 Zeichen lang sein."));
        }

        let cron = schedule::parse(&routine.cron).map_err(|err| format!("\"{name}\": {err}"))?;

        if routine.source == RoutineSource::Create {
            let timezone = server.map_or(chrono_tz::UTC, |s| s.timezone);
            let gap = schedule::min_gap(&cron, timezone, chrono::Utc::now().naive_utc())
                .ok_or_else(|| format!("\"{name}\": Der Zeitplan hat keine zukünftigen Termine."))?;
            // 1 Minute Toleranz für Sommerzeit-Umstellungen
            let min = chrono::Duration::hours(settings.min_interval_hours as i64)
                - chrono::Duration::minutes(1);
            if gap < min {
                return Err(format!(
                    "\"{name}\": Backups dürfen höchstens alle {} Stunde(n) erstellt werden.",
                    settings.min_interval_hours
                ));
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
                return Err(format!(
                    "\"{}\": Bitte eine Quell-Routine auswählen, aus der Backups übernommen werden.",
                    routine.name
                ));
            };
            if !seen.insert(source.uuid) {
                return Err(format!(
                    "\"{}\": Die Quell-Routinen bilden einen Kreis.",
                    routine.name
                ));
            }
            if source.source == RoutineSource::Create {
                break;
            }
            current = source;
        }
    }

    let keep_total: i32 = routines.iter().map(|r| r.keep_count).sum();
    if keep_total as u32 > settings.max_keep_total {
        return Err(format!(
            "Es dürfen höchstens {} Backups pro Server behalten werden (aktuell {keep_total}).",
            settings.max_keep_total
        ));
    }
    if let Some(server) = server
        && keep_total > server.backup_limit
    {
        return Err(format!(
            "Das Backup-Limit dieses Servers ist {}, die Routinen bräuchten aber {keep_total} Plätze.",
            server.backup_limit
        ));
    }

    Ok(())
}
