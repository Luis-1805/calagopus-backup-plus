use indexmap::IndexMap;
use shared::{
    State,
    extensions::{
        Extension, ExtensionPermissionsBuilder, ExtensionRouteBuilder,
        background_tasks::BackgroundTaskBuilder,
    },
    permissions::PermissionGroup,
};
use std::{str::FromStr, sync::Arc};

mod engine;
mod legacy;
mod i18n;
mod models;
mod retention;
mod schedule;
mod templates;
mod validation;
mod routes;
mod settings;
#[cfg(test)]
mod simulation_tests;

pub const PACKAGE_NAME: &str = "dev.backupsplus.backups";

#[derive(Default)]
pub struct ExtensionStruct;

#[async_trait::async_trait]
impl Extension for ExtensionStruct {
    async fn initialize(&mut self, _state: State) {
        tracing::info!("backups-plus: extension loaded");
    }

    async fn initialize_router(
        &mut self,
        state: State,
        builder: ExtensionRouteBuilder,
    ) -> ExtensionRouteBuilder {
        builder
            .add_client_server_api_router(|router| {
                router.nest("/backups-plus", routes::server::router(&state))
            })
            .add_admin_api_router(|router| {
                router.nest(
                    "/extensions/dev.backupsplus.backups",
                    routes::admin::router(&state),
                )
            })
    }

    async fn initialize_background_tasks(
        &mut self,
        _state: State,
        builder: BackgroundTaskBuilder,
    ) -> BackgroundTaskBuilder {
        // Alle 5 Sekunden (6-Feld-Cron mit Sekunden, UTC).
        builder
            .add_cron_task(
                "backups_plus_tick",
                croner::Cron::from_str("*/5 * * * * *").expect("valid cron expression"),
                |state| async move { engine::tick(state).await },
            )
            .await;

        builder
    }

    async fn initialize_permissions(
        &mut self,
        _state: State,
        builder: ExtensionPermissionsBuilder,
    ) -> ExtensionPermissionsBuilder {
        builder.add_server_permission_group(
            "backups-plus",
            PermissionGroup {
                description: "Permissions for automatic backups (Backups Plus).",
                permissions: IndexMap::from([
                    (
                        "read",
                        "Allows viewing the automatic backup policy, managed backups and the run log.",
                    ),
                    (
                        "manage",
                        "Allows changing the automatic backup policy, triggering backups and pinning backups.",
                    ),
                ]),
            },
        )
    }

    async fn settings_deserializer(
        &self,
        _state: State,
    ) -> shared::extensions::settings::ExtensionSettingsDeserializer {
        Arc::new(settings::ExtensionSettingsDataDeserializer)
    }
}

#[cfg(test)]
mod version_tests {
    /// Die Oberfläche kennt ihre eigene Version (frontend/src/version.ts). Sie muss zur
    /// Crate-Version passen, sonst meldet der Server-Tab fälschlich eine veraltete Oberfläche.
    #[test]
    fn frontend_version_matches_crate_version() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // Repository: backend/ und frontend/ nebeneinander; im Panel: frontend/ im Crate-Ordner.
        let candidates = [
            manifest.join("../frontend/src/version.ts"),
            manifest.join("frontend/src/version.ts"),
        ];
        let Some(source) = candidates.iter().find_map(|p| std::fs::read_to_string(p).ok()) else {
            panic!("frontend/src/version.ts not found");
        };

        let expected = format!("'{}'", env!("CARGO_PKG_VERSION"));
        assert!(
            source.contains(&expected),
            "frontend/src/version.ts must contain {expected}"
        );
    }
}
