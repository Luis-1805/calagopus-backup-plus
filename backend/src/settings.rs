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
    /// Kleinstes erlaubtes Intervall (Stunden) zwischen zwei automatischen Backups.
    pub min_interval_hours: u32,
    /// Maximale Gesamtzahl an Backups, die eine Richtlinie behalten darf.
    pub max_keep_total: u32,
    /// Wie viele Server pro Minute höchstens ein neues Backup bekommen (Lastverteilung).
    pub max_backups_per_tick: u32,
    /// Wie viele Protokolleinträge pro Server aufgehoben werden.
    pub log_retention: u32,
}

impl Default for ExtensionSettingsData {
    fn default() -> Self {
        Self {
            enabled: true,
            min_interval_hours: 1,
            max_keep_total: 24,
            max_backups_per_tick: 10,
            log_retention: 100,
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
                "min_interval_hours",
                self.min_interval_hours.to_compact_string(),
            )
            .write_raw_setting("max_keep_total", self.max_keep_total.to_compact_string())
            .write_raw_setting(
                "max_backups_per_tick",
                self.max_backups_per_tick.to_compact_string(),
            )
            .write_raw_setting("log_retention", self.log_retention.to_compact_string()))
    }
}

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
            min_interval_hours: deserializer
                .take_raw_setting("min_interval_hours")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.min_interval_hours)
                .max(1),
            max_keep_total: deserializer
                .take_raw_setting("max_keep_total")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_keep_total)
                .max(1),
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
        }))
    }
}

/// Liest eine Kopie der aktuellen Einstellungen (fällt bei Fehlern auf Standardwerte zurück).
pub async fn load(state: &shared::State) -> ExtensionSettingsData {
    match state.settings.get().await {
        Ok(settings) => match settings.find_extension_settings::<ExtensionSettingsData>() {
            Ok(data) => data.clone(),
            Err(err) => {
                tracing::warn!("backup-plus: Einstellungen nicht gefunden: {err:#?}");
                ExtensionSettingsData::default()
            }
        },
        Err(err) => {
            tracing::warn!("backup-plus: Einstellungen konnten nicht geladen werden: {err:#?}");
            ExtensionSettingsData::default()
        }
    }
}
