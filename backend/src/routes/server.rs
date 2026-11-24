//! Routen unter `/api/client/servers/{server}/backup-plus`.

use shared::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Gemeinsame API-Typen.
mod types {
    use crate::{
        models::{LogEntry, Policy, Tier},
        retention::PlannedAction,
    };
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    pub struct ApiPolicy {
        pub exists: bool,
        pub enabled: bool,
        pub interval_hours: i32,
        pub minute_offset: i32,
        pub keep_hourly: i32,
        pub keep_daily: i32,
        pub keep_weekly: i32,
        pub keep_monthly: i32,
        pub name_template: String,
        pub ignored_files: Vec<String>,
        pub delete_failed: bool,
        pub backup_group_uuid: Option<uuid::Uuid>,
        pub next_run: Option<chrono::DateTime<chrono::Utc>>,
        pub last_run: Option<chrono::DateTime<chrono::Utc>>,
        pub last_error: Option<String>,
    }

    impl ApiPolicy {
        pub fn from_policy(policy: Policy, exists: bool) -> Self {
            Self {
                exists,
                enabled: policy.enabled,
                interval_hours: policy.interval_hours,
                minute_offset: policy.minute_offset,
                keep_hourly: policy.keep_hourly,
                keep_daily: policy.keep_daily,
                keep_weekly: policy.keep_weekly,
                keep_monthly: policy.keep_monthly,
                name_template: policy.name_template,
                ignored_files: policy.ignored_files,
                delete_failed: policy.delete_failed,
                backup_group_uuid: policy.backup_group_uuid,
                next_run: policy.next_run.map(|t| t.and_utc()),
                last_run: policy.last_run.map(|t| t.and_utc()),
                last_error: policy.last_error,
            }
        }
    }

    #[derive(ToSchema, Serialize)]
    pub struct ApiManagedBackup {
        pub uuid: uuid::Uuid,
        pub name: String,
        pub tier: Tier,
        pub successful: bool,
        pub locked: bool,
        pub bytes: i64,
        pub completed: Option<chrono::DateTime<chrono::Utc>>,
        pub created: chrono::DateTime<chrono::Utc>,
        pub planned: PlannedAction,
    }

    #[derive(ToSchema, Serialize)]
    pub struct ApiLimits {
        pub globally_enabled: bool,
        pub min_interval_hours: u32,
        pub max_keep_total: u32,
        pub backup_limit: i32,
        pub backups_total: i64,
    }

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        pub policy: ApiPolicy,
        pub backups: Vec<ApiManagedBackup>,
        pub log: Vec<LogEntry>,
        pub limits: ApiLimits,
    }
}

/// Baut die komplette Übersicht für einen Server.
async fn build_overview(
    state: &State,
    server: &shared::models::server::Server,
) -> Result<types::Response, anyhow::Error> {
    use crate::{
        models::{ManagedBackup, Policy, read_log},
        retention::{self, PlannedAction},
    };

    let settings = crate::settings::load(state).await;
    let stored = Policy::by_server_uuid(&state.database, server.uuid).await?;
    let exists = stored.is_some();
    let policy = stored.unwrap_or_else(|| Policy::default_for(server.uuid));

    let mut backups = Vec::new();
    if let Some(group_uuid) = policy.backup_group_uuid {
        let managed = ManagedBackup::all_for_group(&state.database, server.uuid, group_uuid).await?;
        let plan: std::collections::HashMap<_, _> =
            retention::plan(&managed, &policy, 0).into_iter().collect();

        for backup in managed {
            backups.push(types::ApiManagedBackup {
                uuid: backup.uuid,
                planned: plan
                    .get(&backup.uuid)
                    .cloned()
                    .unwrap_or(PlannedAction::Keep { tier: backup.tier }),
                name: backup.name,
                tier: backup.tier,
                successful: backup.successful,
                locked: backup.locked,
                bytes: backup.bytes,
                completed: backup.completed.map(|t| t.and_utc()),
                created: backup.created.and_utc(),
            });
        }
    }

    let log = read_log(&state.database, server.uuid, 50).await?;
    let backups_total =
        shared::models::server_backup::ServerBackup::count_by_server_uuid(&state.database, server.uuid)
            .await?;

    Ok(types::Response {
        policy: types::ApiPolicy::from_policy(policy, exists),
        backups,
        log,
        limits: types::ApiLimits {
            globally_enabled: settings.enabled,
            min_interval_hours: settings.min_interval_hours,
            max_keep_total: settings.max_keep_total,
            backup_limit: server.backup_limit,
            backups_total,
        },
    })
}

mod get {
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(super::types::Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("backup-plus.read")?;

        ApiResponse::new_serialized(super::build_overview(&state, &server).await?).ok()
    }
}

mod put {
    use crate::{engine, models::Policy};
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        enabled: bool,
        #[garde(range(min = 1, max = 168))]
        #[schema(minimum = 1, maximum = 168)]
        interval_hours: i32,
        #[garde(range(min = 0, max = 59))]
        #[schema(minimum = 0, maximum = 59)]
        minute_offset: i32,
        #[garde(range(min = 1, max = 100))]
        #[schema(minimum = 1, maximum = 100)]
        keep_hourly: i32,
        #[garde(range(min = 0, max = 100))]
        #[schema(minimum = 0, maximum = 100)]
        keep_daily: i32,
        #[garde(range(min = 0, max = 100))]
        #[schema(minimum = 0, maximum = 100)]
        keep_weekly: i32,
        #[garde(range(min = 0, max = 100))]
        #[schema(minimum = 0, maximum = 100)]
        keep_monthly: i32,
        #[garde(length(chars, min = 1, max = 200))]
        #[schema(min_length = 1, max_length = 200)]
        name_template: String,
        #[garde(length(max = 100), inner(length(chars, max = 255)))]
        #[schema(max_items = 100)]
        ignored_files: Vec<String>,
        #[garde(skip)]
        delete_failed: bool,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(super::types::Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("backup-plus.manage")?;

        let settings = crate::settings::load(&state).await;

        if (data.interval_hours as u32) < settings.min_interval_hours {
            return ApiResponse::error(format!(
                "Das Intervall muss mindestens {} Stunde(n) betragen.",
                settings.min_interval_hours
            ))
            .ok();
        }

        let keep_total = data.keep_hourly + data.keep_daily + data.keep_weekly + data.keep_monthly;
        if keep_total as u32 > settings.max_keep_total {
            return ApiResponse::error(format!(
                "Es dürfen höchstens {} Backups behalten werden (aktuell {keep_total}).",
                settings.max_keep_total
            ))
            .ok();
        }
        if keep_total > server.backup_limit {
            return ApiResponse::error(format!(
                "Das Backup-Limit dieses Servers ist {}, die Richtlinie bräuchte aber {keep_total} Plätze.",
                server.backup_limit
            ))
            .ok();
        }

        let previous = Policy::by_server_uuid(&state.database, server.uuid).await?;
        let was_enabled = previous.as_ref().is_some_and(|p| p.enabled);
        let schedule_changed = previous.as_ref().is_some_and(|p| {
            p.interval_hours != data.interval_hours || p.minute_offset != data.minute_offset
        });
        let mut policy = previous.unwrap_or_else(|| Policy::default_for(server.uuid));

        policy.enabled = data.enabled;
        policy.interval_hours = data.interval_hours;
        policy.minute_offset = data.minute_offset;
        policy.keep_hourly = data.keep_hourly;
        policy.keep_daily = data.keep_daily;
        policy.keep_weekly = data.keep_weekly;
        policy.keep_monthly = data.keep_monthly;
        policy.name_template = data.name_template.trim().to_string();
        policy.ignored_files = data
            .ignored_files
            .into_iter()
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect();
        policy.delete_failed = data.delete_failed;

        if policy.enabled && !was_enabled {
            // Beim Aktivieren sofort (im nächsten Minuten-Tick) das erste Backup erstellen.
            policy.next_run = None;
            policy.last_error = None;
        } else if schedule_changed {
            policy.next_run = Some(engine::next_run_after(&policy, engine::now()));
        }

        policy.upsert(&state.database).await?;

        activity_logger
            .log(
                "server:backup-plus.update",
                serde_json::json!({
                    "enabled": policy.enabled,
                    "interval_hours": policy.interval_hours,
                    "keep_hourly": policy.keep_hourly,
                    "keep_daily": policy.keep_daily,
                    "keep_weekly": policy.keep_weekly,
                    "keep_monthly": policy.keep_monthly,
                }),
            )
            .await;

        ApiResponse::new_serialized(super::build_overview(&state, &server).await?).ok()
    }
}

mod run {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod post {
        use crate::{engine, models::Policy};
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::GetPermissionManager,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            backup_uuid: Option<uuid::Uuid>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), params(
            (
                "server" = uuid::Uuid,
                description = "The server ID",
                example = "123e4567-e89b-12d3-a456-426614174000",
            ),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
        ) -> ApiResponseResult {
            permissions.has_server_permission("backup-plus.manage")?;

            state
                .cache
                .ratelimit("backup-plus/run", 3, 60, server.uuid.to_string())
                .await?;

            let mut policy = match Policy::by_server_uuid(&state.database, server.uuid).await? {
                Some(policy) => policy,
                None => {
                    let policy = Policy::default_for(server.uuid);
                    policy.upsert(&state.database).await?;
                    policy
                }
            };

            let backup_uuid = match engine::run_policy(&state, &mut policy, true).await {
                Ok(uuid) => uuid,
                Err(err) => {
                    crate::models::write_log(
                        &state.database,
                        server.uuid,
                        "error",
                        format!("Manueller Lauf fehlgeschlagen: {err:#}"),
                    )
                    .await;
                    return ApiResponse::error(format!("{err:#}")).ok();
                }
            };

            activity_logger
                .log(
                    "server:backup-plus.run",
                    serde_json::json!({ "backup_uuid": backup_uuid }),
                )
                .await;

            ApiResponse::new_serialized(Response { backup_uuid }).ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(post::route))
            .with_state(state.clone())
    }
}

mod backup {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    /// Lädt ein Backup und prüft, dass es zur verwalteten Gruppe gehört.
    async fn find_managed(
        state: &State,
        server_uuid: uuid::Uuid,
        backup_uuid: uuid::Uuid,
    ) -> Result<shared::models::server_backup::ServerBackup, anyhow::Error> {
        use axum::http::StatusCode;
        use shared::response::DisplayError;

        let policy = crate::models::Policy::by_server_uuid(&state.database, server_uuid).await?;
        let backup = shared::models::server_backup::ServerBackup::by_server_uuid_uuid(
            &state.database,
            server_uuid,
            backup_uuid,
        )
        .await?;

        match (policy, backup) {
            (Some(policy), Some(backup))
                if backup.deleted.is_none()
                    && policy.backup_group_uuid.is_some()
                    && backup.backup_group_uuid == policy.backup_group_uuid =>
            {
                Ok(backup)
            }
            _ => Err(DisplayError::new("Backup nicht gefunden")
                .with_status(StatusCode::NOT_FOUND)
                .into()),
        }
    }

    mod pin {
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{
                UpdatableModel,
                server::{GetServer, GetServerActivityLogger},
                server_backup::UpdateServerBackupOptions,
                user::GetPermissionManager,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Deserialize)]
        pub struct Payload {
            locked: bool,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {}

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = NOT_FOUND, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), params(
            ("server" = uuid::Uuid, description = "The server ID"),
            ("backup" = uuid::Uuid, description = "The backup ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
            axum::extract::Path((_server, backup_uuid)): axum::extract::Path<(uuid::Uuid, uuid::Uuid)>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            permissions.has_server_permission("backup-plus.manage")?;

            let mut backup = super::find_managed(&state, server.uuid, backup_uuid).await?;
            backup
                .update(
                    &state,
                    UpdateServerBackupOptions {
                        locked: Some(data.locked),
                        ..Default::default()
                    },
                )
                .await?;

            crate::models::write_log(
                &state.database,
                server.uuid,
                "info",
                if data.locked {
                    format!("\"{}\" angepinnt (wird nie automatisch gelöscht)", backup.name)
                } else {
                    format!("\"{}\" losgelöst (unterliegt wieder der Rotation)", backup.name)
                },
            )
            .await;

            activity_logger
                .log(
                    "server:backup-plus.pin",
                    serde_json::json!({ "uuid": backup.uuid, "locked": data.locked }),
                )
                .await;

            ApiResponse::new_serialized(Response {}).ok()
        }
    }

    mod tier {
        use crate::models::{self, Tier};
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{server::GetServer, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Deserialize)]
        pub struct Payload {
            tier: Tier,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {}

        #[utoipa::path(put, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = NOT_FOUND, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), params(
            ("server" = uuid::Uuid, description = "The server ID"),
            ("backup" = uuid::Uuid, description = "The backup ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            axum::extract::Path((_server, backup_uuid)): axum::extract::Path<(uuid::Uuid, uuid::Uuid)>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            permissions.has_server_permission("backup-plus.manage")?;

            let backup = super::find_managed(&state, server.uuid, backup_uuid).await?;
            models::set_tier(&state.database, server.uuid, backup.uuid, data.tier, true).await?;
            models::write_log(
                &state.database,
                server.uuid,
                "info",
                format!(
                    "\"{}\" manuell der Stufe {} zugeordnet",
                    backup.name,
                    data.tier.label()
                ),
            )
            .await;

            ApiResponse::new_serialized(Response {}).ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .nest(
                "/pin",
                OpenApiRouter::new()
                    .routes(routes!(pin::route))
                    .with_state(state.clone()),
            )
            .nest(
                "/tier",
                OpenApiRouter::new()
                    .routes(routes!(tier::route))
                    .with_state(state.clone()),
            )
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .nest("/run", run::router(state))
        .nest("/backups/{backup}", backup::router(state))
        .with_state(state.clone())
}
