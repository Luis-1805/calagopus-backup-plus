//! Übergang von der alten Paket-ID `dev.backupplus.gfsbackups` (bis v2.3.0) zu `dev.backupsplus.backups`.
//!
//! Die eigentliche Übernahme (Tabellen, Einstellungen, Rechte) erledigt die Migration
//! `20261125210000_rename_to_backups_plus`. Dieses Modul verhindert nur, dass alte und neue
//! Extension gleichzeitig Backups anlegen, falls beide installiert sind.
//! Kann entfernt werden, sobald keine Installation mehr die alte Paket-ID nutzt.

use shared::State;
use std::sync::atomic::{AtomicBool, Ordering};

pub const OLD_PACKAGE_NAME: &str = "dev.backupplus.gfsbackups";

static WARNED: AtomicBool = AtomicBool::new(false);

/// `true`, solange die alte Extension geladen und aktiv ist.
pub async fn old_extension_active(state: &State) -> bool {
    let active = state.extensions.extensions().await.iter().any(|ext| {
        ext.package_name == OLD_PACKAGE_NAME
            && !state.extensions.is_disabled(ext.package_name)
            && !state.extensions.is_failed(ext.package_name)
    });

    if active && !WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            "backups-plus: the old extension {OLD_PACKAGE_NAME} is still installed – automatic backups are paused until it is removed (without rolling back its migrations)"
        );
    }

    active
}
