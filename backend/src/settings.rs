use crate::templates::{DEFAULT_TEMPLATE_ID, Template};
use compact_str::ToCompactString;
use serde::{Deserialize, Serialize};
use shared::extensions::settings::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use utoipa::ToSchema;

/// Globale, vom Admin konfigurierbare Einstellungen der Extension.
#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct ExtensionSettingsData {
    /// Hauptschalter: wenn `false`, erstellt und löscht die Extension nichts.
    pub enabled: bool,
    /// Kleinster erlaubter Abstand (Minuten) zwischen zwei neu erstellten Backups einer Routine.
    pub min_interval_minutes: u32,
    /// Maximale Gesamtzahl an Backups, die alle Routinen eines Servers behalten dürfen.
    pub max_keep_total: u32,
    /// Maximale Anzahl Routinen pro Server.
    pub max_routines: u32,
    /// Dürfen Server-Nutzer eigene Routinen anlegen? Sonst nur Vorlagen anwenden.
    pub allow_custom_routines: bool,
    /// Vorlage, die für noch nicht eingerichtete Server vorgeschlagen wird.
    pub default_template_id: String,
    /// Zeitzone für Server ohne eigene Zeitzone, wenn auch der Node keine meldet (leer = UTC).
    pub default_timezone: String,
    /// Wie viele Server pro Minute höchstens bearbeitet werden (Lastverteilung).
    pub max_backups_per_tick: u32,
    /// Wie viele Protokolleinträge pro Server aufgehoben werden.
    pub log_retention: u32,
    /// Eigene Vorlagen des Admins (zusätzlich zu den eingebauten).
    pub templates: Vec<Template>,
}

impl Default for ExtensionSettingsData {
    fn default() -> Self {
        Self {
            enabled: true,
            min_interval_minutes: 60,
            max_keep_total: 50,
            max_routines: 10,
            allow_custom_routines: true,
            default_template_id: DEFAULT_TEMPLATE_ID.into(),
            default_timezone: String::new(),
            max_backups_per_tick: 10,
            log_retention: 100,
            templates: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for ExtensionSettingsData {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting(
                "min_interval_minutes",
                self.min_interval_minutes.to_compact_string(),
            )
            .write_raw_setting("max_keep_total", self.max_keep_total.to_compact_string())
            .write_raw_setting("max_routines", self.max_routines.to_compact_string())
            .write_raw_setting(
                "allow_custom_routines",
                self.allow_custom_routines.to_compact_string(),
            )
            .write_raw_setting(
                "default_template_id",
                self.default_template_id.to_compact_string(),
            )
            .write_raw_setting(
                "max_backups_per_tick",
                self.max_backups_per_tick.to_compact_string(),
            )
            .write_raw_setting("default_timezone", self.default_timezone.to_compact_string())
            .write_raw_setting("log_retention", self.log_retention.to_compact_string())
            .write_serde_setting("templates", &self.templates)?)
    }
}

pub const MIN_INTERVAL_MINUTES: u32 = 5;
pub const MAX_INTERVAL_MINUTES: u32 = 10_080;

pub struct ExtensionSettingsDataDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for ExtensionSettingsDataDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = ExtensionSettingsData::default();

        Ok(Box::new(ExtensionSettingsData {
            enabled: deserializer
                .take_raw_setting("enabled")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.enabled),
            // Ab 2.3 in Minuten; ältere Installationen haben den Wert in Stunden gespeichert.
            min_interval_minutes: deserializer
                .take_raw_setting("min_interval_minutes")
                .and_then(|s| s.parse::<u32>().ok())
                .or_else(|| {
                    deserializer
                        .take_raw_setting("min_interval_hours")
                        .and_then(|s| s.parse::<u32>().ok())
                        .map(|hours| hours.saturating_mul(60))
                })
                .unwrap_or(defaults.min_interval_minutes)
                .clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES),
            max_keep_total: deserializer
                .take_raw_setting("max_keep_total")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_keep_total)
                .max(1),
            max_routines: deserializer
                .take_raw_setting("max_routines")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_routines)
                .max(1),
            allow_custom_routines: deserializer
                .take_raw_setting("allow_custom_routines")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.allow_custom_routines),
            default_template_id: deserializer
                .take_raw_setting("default_template_id")
                .map(|s| s.to_string())
                .unwrap_or(defaults.default_template_id),
            default_timezone: deserializer
                .take_raw_setting("default_timezone")
                .map(|s| s.to_string())
                .filter(|s| s.is_empty() || s.parse::<chrono_tz::Tz>().is_ok())
                .unwrap_or_default(),
            max_backups_per_tick: deserializer
                .take_raw_setting("max_backups_per_tick")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_backups_per_tick)
                .max(1),
            log_retention: deserializer
                .take_raw_setting("log_retention")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.log_retention)
                .max(10),
            templates: deserializer
                .read_serde_setting("templates")
                .unwrap_or_default(),
        }))
    }
}

/// Liest eine Kopie der aktuellen Einstellungen (fällt bei Fehlern auf Standardwerte zurück).
pub async fn load(state: &shared::State) -> ExtensionSettingsData {
    match state.settings.get().await {
        Ok(settings) => match settings.find_extension_settings::<ExtensionSettingsData>() {
            Ok(data) => data.clone(),
            Err(err) => {
                tracing::warn!("backups-plus: settings not found: {err:#?}");
                ExtensionSettingsData::default()
            }
        },
        Err(err) => {
            tracing::warn!("backups-plus: failed to load settings: {err:#?}");
            ExtensionSettingsData::default()
        }
    }
}

impl ExtensionSettingsData {
    pub fn all_templates(&self, lang: crate::i18n::Lang) -> Vec<Template> {
        crate::templates::all(&self.templates, lang)
    }

    pub fn find_template(&self, id: &str, lang: crate::i18n::Lang) -> Option<Template> {
        self.all_templates(lang).into_iter().find(|t| t.id == id)
    }
}
