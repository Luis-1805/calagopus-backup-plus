//! Vorlagen für Backup-Routinen: eingebaute und vom Admin angelegte.

use crate::models::{Routine, RoutineSource};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Deserialize, Validate, Clone, Debug)]
pub struct TemplateRoutine {
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub name: String,
    #[serde(default = "default_true")]
    #[garde(skip)]
    pub enabled: bool,
    #[garde(length(chars, min = 1, max = 128))]
    #[schema(min_length = 1, max_length = 128)]
    pub cron: String,
    #[garde(skip)]
    pub source: RoutineSource,
    /// Name der Quell-Routine innerhalb derselben Vorlage (nur bei `promote`).
    #[serde(default)]
    #[garde(inner(length(chars, max = 64)))]
    pub source_name: Option<String>,
    #[garde(range(min = 1, max = 500))]
    #[schema(minimum = 1, maximum = 500)]
    pub keep_count: i32,
    #[serde(default)]
    #[garde(range(min = 0, max = 87600))]
    #[schema(minimum = 0, maximum = 87600)]
    pub max_age_hours: i32,
    #[serde(default = "default_name_template")]
    #[garde(length(chars, min = 1, max = 200))]
    pub name_template: String,
    #[serde(default)]
    #[garde(length(max = 100), inner(length(chars, max = 255)))]
    pub ignored_files: Vec<String>,
}

#[derive(ToSchema, Serialize, Deserialize, Validate, Clone, Debug)]
pub struct Template {
    #[garde(length(chars, min = 1, max = 64), custom(valid_id))]
    #[schema(min_length = 1, max_length = 64, pattern = r"^[a-z0-9][a-z0-9-]*$")]
    pub id: String,
    #[garde(length(chars, min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    #[serde(default)]
    #[garde(length(chars, max = 500))]
    pub description: String,
    /// Eingebaute Vorlagen können nicht verändert werden.
    #[serde(default)]
    #[garde(skip)]
    pub builtin: bool,
    #[garde(length(min = 1, max = 20), dive)]
    #[schema(min_items = 1, max_items = 20)]
    pub routines: Vec<TemplateRoutine>,
}

fn valid_id(value: &str, _context: &()) -> garde::Result {
    let ok = value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !value.starts_with('-');

    if ok {
        Ok(())
    } else {
        Err(garde::Error::new(
            "nur Kleinbuchstaben, Ziffern und Bindestriche erlaubt",
        ))
    }
}

fn default_true() -> bool {
    true
}

pub fn default_name_template() -> String {
    "Auto {routine} {date}".into()
}

fn routine(
    name: &str,
    cron: &str,
    source_name: Option<&str>,
    keep_count: i32,
    max_age_hours: i32,
) -> TemplateRoutine {
    TemplateRoutine {
        name: name.into(),
        enabled: true,
        cron: cron.into(),
        source: if source_name.is_some() {
            RoutineSource::Promote
        } else {
            RoutineSource::Create
        },
        source_name: source_name.map(Into::into),
        keep_count,
        max_age_hours,
        name_template: default_name_template(),
        ignored_files: Vec::new(),
    }
}

pub const DEFAULT_TEMPLATE_ID: &str = "gfs-kompakt";

/// Eingebaute Vorlagen.
pub fn builtin() -> Vec<Template> {
    vec![
        Template {
            id: DEFAULT_TEMPLATE_ID.into(),
            name: "GFS kompakt (5 Backups)".into(),
            description: "Die letzten 3 Stunden, dazu ein wöchentliches (jeden Montag erneuert) und ein monatliches (jeden 1. erneuert) Backup. Ältere werden gelöscht.".into(),
            builtin: true,
            routines: vec![
                routine("Stündlich", "0 0 * * * *", None, 3, 0),
                routine("Wöchentlich", "0 0 0 * * 1", Some("Stündlich"), 1, 0),
                routine("Monatlich", "0 0 0 1 * *", Some("Stündlich"), 1, 0),
            ],
        },
        Template {
            id: "gfs-erweitert".into(),
            name: "GFS erweitert (41 Backups)".into(),
            description: "24 stündliche, 7 tägliche, 4 wöchentliche und 6 monatliche Backups.".into(),
            builtin: true,
            routines: vec![
                routine("Stündlich", "0 0 * * * *", None, 24, 0),
                routine("Täglich", "0 0 0 * * *", Some("Stündlich"), 7, 0),
                routine("Wöchentlich", "0 0 0 * * 1", Some("Täglich"), 4, 0),
                routine("Monatlich", "0 0 0 1 * *", Some("Wöchentlich"), 6, 0),
            ],
        },
        Template {
            id: "taeglich-7".into(),
            name: "Täglich, 7 Tage".into(),
            description: "Jede Nacht um 04:00 ein Backup, die letzten 7 werden behalten.".into(),
            builtin: true,
            routines: vec![routine("Täglich", "0 0 4 * * *", None, 7, 0)],
        },
        Template {
            id: "alle-6-stunden".into(),
            name: "Alle 6 Stunden, 2 Tage".into(),
            description: "Alle 6 Stunden ein Backup, maximal 2 Tage alt.".into(),
            builtin: true,
            routines: vec![routine("Alle 6 Stunden", "0 0 */6 * * *", None, 8, 48)],
        },
    ]
}

/// Eingebaute und eigene Vorlagen zusammen.
pub fn all(custom: &[Template]) -> Vec<Template> {
    let mut templates = builtin();
    templates.extend(custom.iter().cloned().map(|mut t| {
        t.builtin = false;
        t
    }));
    templates
}

/// Baut aus einer Vorlage die Routinen für einen Server (Quellen per Name aufgelöst).
pub fn to_routines(template: &Template, server_uuid: uuid::Uuid) -> Result<Vec<Routine>, String> {
    let uuids: Vec<uuid::Uuid> = template.routines.iter().map(|_| uuid::Uuid::new_v4()).collect();

    template
        .routines
        .iter()
        .enumerate()
        .map(|(index, r)| {
            let source_routine_uuid = match (r.source, &r.source_name) {
                (RoutineSource::Promote, Some(source)) => Some(
                    template
                        .routines
                        .iter()
                        .position(|other| &other.name == source)
                        .map(|i| uuids[i])
                        .ok_or_else(|| {
                            format!("Routine \"{}\": Quell-Routine \"{source}\" fehlt in der Vorlage", r.name)
                        })?,
                ),
                (RoutineSource::Promote, None) => {
                    return Err(format!("Routine \"{}\": Quell-Routine fehlt", r.name));
                }
                (RoutineSource::Create, _) => None,
            };

            Ok(Routine {
                uuid: uuids[index],
                server_uuid,
                name: r.name.trim().to_string(),
                enabled: r.enabled,
                position: index as i32,
                cron: r.cron.trim().to_string(),
                source: r.source,
                source_routine_uuid,
                keep_count: r.keep_count,
                max_age_hours: r.max_age_hours,
                name_template: r.name_template.trim().to_string(),
                ignored_files: r.ignored_files.clone(),
                next_run: None,
                last_run: None,
                last_error: None,
            })
        })
        .collect()
}
