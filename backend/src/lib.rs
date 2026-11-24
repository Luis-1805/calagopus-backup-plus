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
mod models;
mod retention;
mod routes;
mod settings;

pub const PACKAGE_NAME: &str = "dev.backupplus.gfsbackups";

#[derive(Default)]
pub struct ExtensionStruct;

#[async_trait::async_trait]
impl Extension for ExtensionStruct {
    async fn initialize(&mut self, _state: State) {
        tracing::info!("backup-plus: Extension geladen");
    }

    async fn initialize_router(
        &mut self,
        state: State,
        builder: ExtensionRouteBuilder,
    ) -> ExtensionRouteBuilder {
        builder
            .add_client_server_api_router(|router| {
                router.nest("/backup-plus", routes::server::router(&state))
            })
            .add_admin_api_router(|router| {
                router.nest(
                    "/extensions/dev.backupplus.gfsbackups",
                    routes::admin::router(&state),
                )
            })
    }

    async fn initialize_background_tasks(
        &mut self,
        _state: State,
        builder: BackgroundTaskBuilder,
    ) -> BackgroundTaskBuilder {
        // Jede Minute (6-Feld-Cron mit Sekunden, UTC).
        builder
            .add_cron_task(
                "backup_plus_tick",
                croner::Cron::from_str("0 * * * * *").expect("gültiger Cron-Ausdruck"),
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
            "backup-plus",
            PermissionGroup {
                description: "Berechtigungen für automatische Backups (Backup Plus).",
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
