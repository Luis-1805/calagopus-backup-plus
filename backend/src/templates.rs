//! Vorlagen für Backup-Routinen: eingebaute und vom Admin angelegte.

use crate::{
    i18n::{Lang, Msg, t},
    models::{Routine, RoutineSource},
};
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
    /// Optional: neuer Name für übernommene Backups (nur bei `promote`).
    #[serde(default)]
    #[garde(inner(length(chars, max = 200)))]
    pub rename_template: Option<String>,
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
            "only lowercase letters, digits and hyphens are allowed",
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
        rename_template: None,
        ignored_files: Vec::new(),
    }
}

/// Die ID ist intern und bleibt aus Kompatibilitätsgründen unverändert.
pub const DEFAULT_TEMPLATE_ID: &str = "gfs-kompakt";

/// Eingebaute Vorlagen, Namen in der Sprache des Nutzers.
pub fn builtin(lang: Lang) -> Vec<Template> {
    let hourly = t("routine.hourly", lang);
    let daily = t("routine.daily", lang);
    let weekly = t("routine.weekly", lang);
    let monthly = t("routine.monthly", lang);
    let every_6h = t("routine.every_6h", lang);

    vec![
        Template {
            id: DEFAULT_TEMPLATE_ID.into(),
            name: t("tpl.gfs_compact.name", lang),
            description: t("tpl.gfs_compact.description", lang),
            builtin: true,
            routines: vec![
                routine(&hourly, "0 0 * * * *", None, 3, 0),
                routine(&weekly, "0 0 0 * * 1", Some(&hourly), 1, 0),
                routine(&monthly, "0 0 0 1 * *", Some(&hourly), 1, 0),
            ],
        },
        Template {
            id: "gfs-erweitert".into(),
            name: t("tpl.gfs_extended.name", lang),
            description: t("tpl.gfs_extended.description", lang),
            builtin: true,
            routines: vec![
                routine(&hourly, "0 0 * * * *", None, 24, 0),
                routine(&daily, "0 0 0 * * *", Some(&hourly), 7, 0),
                routine(&weekly, "0 0 0 * * 1", Some(&daily), 4, 0),
                routine(&monthly, "0 0 0 1 * *", Some(&weekly), 6, 0),
            ],
        },
        Template {
            id: "taeglich-7".into(),
            name: t("tpl.daily_7.name", lang),
            description: t("tpl.daily_7.description", lang),
            builtin: true,
            routines: vec![routine(&daily, "0 0 4 * * *", None, 7, 0)],
        },
        Template {
            id: "alle-6-stunden".into(),
            name: t("tpl.every_6h.name", lang),
            description: t("tpl.every_6h.description", lang),
            builtin: true,
            routines: vec![routine(&every_6h, "0 0 */6 * * *", None, 8, 48)],
        },
    ]
}

/// Eingebaute und eigene Vorlagen zusammen.
pub fn all(custom: &[Template], lang: Lang) -> Vec<Template> {
    let mut templates = builtin(lang);
    templates.extend(custom.iter().cloned().map(|mut t| {
        t.builtin = false;
        t
    }));
    templates
}

/// Baut aus einer Vorlage die Routinen für einen Server (Quellen per Name aufgelöst).
pub fn to_routines(template: &Template, server_uuid: uuid::Uuid) -> Result<Vec<Routine>, Msg> {
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
                            Msg::new("val.template_source_unknown")
                                .arg("routine", &r.name)
                                .arg("source", source)
                        })?,
                ),
                (RoutineSource::Promote, None) => {
                    return Err(Msg::new("val.template_source_missing").arg("routine", &r.name));
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
                rename_template: r.rename_template.clone(),
                ignored_files: r.ignored_files.clone(),
                next_run: None,
                last_run: None,
                last_error: None,
            })
        })
        .collect()
}
