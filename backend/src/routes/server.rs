//! Routen unter `/api/client/servers/{server}/backup-plus`.

use shared::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Gemeinsame API-Typen.
mod types {
    use crate::{
        models::{LogEntry, Routine, RoutineSource},
        retention::PlannedAction,
        templates::Template,
    };
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    pub struct ApiPolicy {
        pub exists: bool,
        pub enabled: bool,
        pub delete_failed: bool,
        pub last_run: Option<chrono::DateTime<chrono::Utc>>,
        pub last_error: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    pub struct ApiRoutine {
        pub uuid: uuid::Uuid,
        pub name: String,
        pub enabled: bool,
        pub position: i32,
        pub cron: String,
        pub source: RoutineSource,
        pub source_routine_uuid: Option<uuid::Uuid>,
        pub keep_count: i32,
        pub max_age_hours: i32,
        pub name_template: String,
        pub ignored_files: Vec<String>,
        pub next_run: Option<chrono::DateTime<chrono::Utc>>,
        pub last_run: Option<chrono::DateTime<chrono::Utc>>,
        pub last_error: Option<String>,
        pub backup_count: i64,
    }

    impl ApiRoutine {
        pub fn new(routine: Routine, backup_count: i64) -> Self {
            Self {
                uuid: routine.uuid,
                name: routine.name,
                enabled: routine.enabled,
                position: routine.position,
                cron: routine.cron,
                source: routine.source,
                source_routine_uuid: routine.source_routine_uuid,
                keep_count: routine.keep_count,
                max_age_hours: routine.max_age_hours,
                name_template: routine.name_template,
                ignored_files: routine.ignored_files,
                next_run: routine.next_run.map(|t| t.and_utc()),
                last_run: routine.last_run.map(|t| t.and_utc()),
                last_error: routine.last_error,
                backup_count,
            }
        }
    }

    #[derive(ToSchema, Serialize)]
    pub struct ApiManagedBackup {
        pub uuid: uuid::Uuid,
        pub name: String,
        pub routine_uuid: Option<uuid::Uuid>,
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
        pub max_routines: u32,
        pub allow_custom_routines: bool,
        /// Ob der aktuelle Nutzer eigene Routinen bearbeiten darf.
        pub can_customize: bool,
        pub backup_limit: i32,
        pub backups_total: i64,
    }

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        pub policy: ApiPolicy,
        pub routines: Vec<ApiRoutine>,
        pub backups: Vec<ApiManagedBackup>,
        pub log: Vec<LogEntry>,
        pub limits: ApiLimits,
        pub templates: Vec<Template>,
        pub default_template_id: String,
        pub timezone: String,
    }
}

/// Ob der Nutzer eigene Routinen bauen darf (sonst nur Vorlagen anwenden).
fn can_customize(
    settings: &crate::settings::ExtensionSettingsData,
    permissions: &shared::models::user::PermissionManager,
) -> bool {
    settings.allow_custom_routines || permissions.has_admin_permission("extensions.manage").is_ok()
}

/// Baut die komplette Übersicht für einen Server.
async fn build_overview(
    state: &State,
    server: &shared::models::server::Server,
    can_customize: bool,
) -> Result<types::Response, anyhow::Error> {
    use crate::{
        engine,
        models::{ManagedBackup, Policy, Routine, read_log},
        retention::{self, PlannedAction},
        schedule,
    };

    let settings = crate::settings::load(state).await;
    let stored = Policy::by_server_uuid(&state.database, server.uuid).await?;
    let exists = stored.is_some();
    let policy = stored.unwrap_or_else(|| Policy::default_for(server.uuid));
    let routines = Routine::all_for_server(&state.database, server.uuid).await?;

    let managed = match policy.backup_group_uuid {
        Some(group_uuid) => {
            ManagedBackup::all_for_group(&state.database, server.uuid, group_uuid).await?
        }
        None => Vec::new(),
    };

    let now = engine::now();
    let (routine_specs, backup_specs) = engine::build_specs(&routines, &managed, now);
    let plan = retention::plan(&backup_specs, &routine_specs, policy.delete_failed, now, None);

    let backups = managed
        .into_iter()
        .map(|backup| types::ApiManagedBackup {
            uuid: backup.uuid,
            planned: plan
                .action_for(backup.uuid)
                .cloned()
                .unwrap_or(PlannedAction::Unassigned),
            name: backup.name,
            routine_uuid: backup.routine_uuid,
            successful: backup.successful,
            locked: backup.locked,
            bytes: backup.bytes,
            completed: backup.completed.map(|t| t.and_utc()),
            created: backup.created.and_utc(),
        })
        .collect::<Vec<_>>();

    let routines = routines
        .into_iter()
        .map(|routine| {
            let count = backups
                .iter()
                .filter(|b| b.routine_uuid == Some(routine.uuid))
                .count() as i64;
            types::ApiRoutine::new(routine, count)
        })
        .collect();

    let log = read_log(&state.database, server.uuid, 50).await?;
    let backups_total =
        shared::models::server_backup::ServerBackup::count_by_server_uuid(&state.database, server.uuid)
            .await?;

    Ok(types::Response {
        policy: types::ApiPolicy {
            exists,
            enabled: policy.enabled,
            delete_failed: policy.delete_failed,
            last_run: policy.last_run.map(|t| t.and_utc()),
            last_error: policy.last_error,
        },
        routines,
        backups,
        log,
        limits: types::ApiLimits {
            globally_enabled: settings.enabled,
            min_interval_hours: settings.min_interval_hours,
            max_keep_total: settings.max_keep_total,
            max_routines: settings.max_routines,
            allow_custom_routines: settings.allow_custom_routines,
            can_customize,
            backup_limit: server.backup_limit,
            backups_total,
        },
        templates: settings.all_templates(),
        default_template_id: settings.default_template_id.clone(),
        timezone: schedule::server_timezone(server.timezone.as_deref()).name().to_string(),
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

        let settings = crate::settings::load(&state).await;
        let can_customize = super::can_customize(&settings, &permissions);

        ApiResponse::new_serialized(super::build_overview(&state, &server, can_customize).await?).ok()
    }
}

mod put {
    use crate::{
        models::{Policy, Routine, RoutineSource},
        schedule,
        validation::{self, ServerLimits},
    };
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
    use std::collections::HashMap;
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct RoutineInput {
        /// Frei wählbarer Schlüssel, über den andere Routinen diese als Quelle referenzieren.
        #[garde(length(chars, min = 1, max = 64))]
        key: String,
        /// UUID einer bestehenden Routine (fehlt bei neuen Routinen).
        #[garde(skip)]
        uuid: Option<uuid::Uuid>,
        #[garde(length(chars, min = 1, max = 64))]
        #[schema(min_length = 1, max_length = 64)]
        name: String,
        #[garde(skip)]
        enabled: bool,
        #[garde(length(chars, min = 1, max = 128))]
        #[schema(min_length = 1, max_length = 128)]
        cron: String,
        #[garde(skip)]
        source: RoutineSource,
        #[garde(inner(length(chars, max = 64)))]
        source_key: Option<String>,
        #[garde(range(min = 1, max = 500))]
        #[schema(minimum = 1, maximum = 500)]
        keep_count: i32,
        #[garde(range(min = 0, max = 87600))]
        #[schema(minimum = 0, maximum = 87600)]
        max_age_hours: i32,
        #[garde(length(chars, min = 1, max = 200))]
        #[schema(min_length = 1, max_length = 200)]
        name_template: String,
        #[garde(length(max = 100), inner(length(chars, max = 255)))]
        #[schema(max_items = 100)]
        ignored_files: Vec<String>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        enabled: bool,
        #[garde(skip)]
        delete_failed: bool,
        #[garde(length(max = 50), dive)]
        #[schema(max_items = 50)]
        routines: Vec<RoutineInput>,
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
        let can_customize = super::can_customize(&settings, &permissions);
        let timezone = schedule::server_timezone(server.timezone.as_deref());
        let now = crate::engine::now();

        let previous = Policy::by_server_uuid(&state.database, server.uuid).await?;
        let was_enabled = previous.as_ref().is_some_and(|p| p.enabled);
        let mut policy = previous.unwrap_or_else(|| Policy::default_for(server.uuid));
        policy.enabled = data.enabled;
        policy.delete_failed = data.delete_failed;

        let existing = Routine::all_for_server(&state.database, server.uuid).await?;
        let restart = data.enabled && !was_enabled;

        let routines = if can_customize {
            // Schlüssel → UUID (bestehende behalten ihre UUID, neue bekommen eine).
            let mut keys: HashMap<String, uuid::Uuid> = HashMap::new();
            for input in &data.routines {
                let uuid = input
                    .uuid
                    .filter(|uuid| existing.iter().any(|r| r.uuid == *uuid))
                    .unwrap_or_else(uuid::Uuid::new_v4);
                if keys.insert(input.key.clone(), uuid).is_some() {
                    return ApiResponse::error("Doppelter Routinen-Schlüssel.").ok();
                }
            }

            let mut routines = Vec::with_capacity(data.routines.len());
            for (position, input) in data.routines.into_iter().enumerate() {
                let uuid = keys[&input.key];
                let previous = existing.iter().find(|r| r.uuid == uuid);

                let source_routine_uuid = match input.source {
                    RoutineSource::Promote => input
                        .source_key
                        .as_ref()
                        .and_then(|key| keys.get(key))
                        .copied(),
                    RoutineSource::Create => None,
                };

                let cron = input.cron.trim().to_string();
                // Termin behalten, solange sich am Zeitplan nichts geändert hat.
                let next_run = match previous {
                    Some(p) if !restart && p.cron == cron && p.enabled == input.enabled => p.next_run,
                    Some(_) if !restart => schedule::parse(&cron)
                        .ok()
                        .and_then(|c| schedule::next_after(&c, timezone, now)),
                    _ => None,
                };

                routines.push(Routine {
                    uuid,
                    server_uuid: server.uuid,
                    name: input.name.trim().to_string(),
                    enabled: input.enabled,
                    position: position as i32,
                    cron,
                    source: input.source,
                    source_routine_uuid,
                    keep_count: input.keep_count,
                    max_age_hours: input.max_age_hours,
                    name_template: input.name_template.trim().to_string(),
                    ignored_files: input
                        .ignored_files
                        .into_iter()
                        .map(|f| f.trim().to_string())
                        .filter(|f| !f.is_empty())
                        .collect(),
                    next_run,
                    last_run: previous.and_then(|p| p.last_run),
                    last_error: previous.and_then(|p| p.last_error.clone()),
                });
            }

            if let Err(err) = validation::validate(
                &routines,
                &settings,
                Some(&ServerLimits {
                    backup_limit: server.backup_limit,
                    timezone,
                }),
            ) {
                return ApiResponse::error(err).ok();
            }

            Some(routines)
        } else {
            None
        };

        if policy.enabled && routines.as_ref().map_or(existing.is_empty(), Vec::is_empty) {
            return ApiResponse::error(
                "Bitte zuerst mindestens eine Routine anlegen oder eine Vorlage anwenden.",
            )
            .ok();
        }

        policy.last_error = None;
        policy.upsert(&state.database).await?;

        if let Some(routines) = &routines {
            Routine::replace_all(&state.database, server.uuid, routines).await?;
        } else if restart {
            // Ohne eigene Routinen: beim Aktivieren alle Termine neu starten.
            for mut routine in existing {
                routine.next_run = None;
                routine.save_run_state(&state.database).await?;
            }
        }

        activity_logger
            .log(
                "server:backup-plus.update",
                serde_json::json!({
                    "enabled": policy.enabled,
                    "delete_failed": policy.delete_failed,
                    "routines": routines.as_ref().map(|r| r.iter().map(|r| serde_json::json!({
                        "name": r.name,
                        "cron": r.cron,
                        "source": r.source,
                        "keep_count": r.keep_count,
                    })).collect::<Vec<_>>()),
                }),
            )
            .await;

        ApiResponse::new_serialized(super::build_overview(&state, &server, can_customize).await?).ok()
    }
}

mod apply_template {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod post {
        use crate::{
            models::{Policy, Routine},
            schedule, templates,
            validation::{self, ServerLimits},
        };
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

        #[derive(ToSchema, Deserialize)]
        pub struct Payload {
            template_id: String,
            #[serde(default = "default_true")]
            enabled: bool,
        }

        fn default_true() -> bool {
            true
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(super::super::types::Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), params(
            ("server" = uuid::Uuid, description = "The server ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            permissions.has_server_permission("backup-plus.manage")?;

            let settings = crate::settings::load(&state).await;
            let can_customize = super::super::can_customize(&settings, &permissions);
            let Some(template) = settings.find_template(&data.template_id) else {
                return ApiResponse::error("Vorlage nicht gefunden.").ok();
            };

            let routines = match templates::to_routines(&template, server.uuid) {
                Ok(routines) => routines,
                Err(err) => return ApiResponse::error(err).ok(),
            };
            if let Err(err) = validation::validate(
                &routines,
                &settings,
                Some(&ServerLimits {
                    backup_limit: server.backup_limit,
                    timezone: schedule::server_timezone(server.timezone.as_deref()),
                }),
            ) {
                return ApiResponse::error(format!("Vorlage passt nicht: {err}")).ok();
            }

            let mut policy = Policy::by_server_uuid(&state.database, server.uuid)
                .await?
                .unwrap_or_else(|| Policy::default_for(server.uuid));
            policy.enabled = data.enabled;
            policy.last_error = None;
            policy.upsert(&state.database).await?;
            Routine::replace_all(&state.database, server.uuid, &routines).await?;

            crate::models::write_log(
                &state.database,
                server.uuid,
                "info",
                format!("Vorlage \"{}\" angewendet", template.name),
            )
            .await;
            activity_logger
                .log(
                    "server:backup-plus.apply-template",
                    serde_json::json!({ "template": template.id, "enabled": data.enabled }),
                )
                .await;

            ApiResponse::new_serialized(
                super::super::build_overview(&state, &server, can_customize).await?,
            )
            .ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(post::route))
            .with_state(state.clone())
    }
}

mod run {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod post {
        use crate::engine::{self, Trigger};
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::GetPermissionManager,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Deserialize, Default)]
        pub struct Payload {
            #[serde(default)]
            routine_uuid: Option<uuid::Uuid>,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            backup_uuid: Option<uuid::Uuid>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), params(
            ("server" = uuid::Uuid, description = "The server ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            permissions.has_server_permission("backup-plus.manage")?;

            state
                .cache
                .ratelimit("backup-plus/run", 3, 60, server.uuid.to_string())
                .await?;

            let backup_uuid =
                match engine::run_server(&state, server.uuid, Trigger::Manual(data.routine_uuid))
                    .await
                {
                    Ok(Some(uuid)) => Some(uuid),
                    Ok(None) => {
                        return ApiResponse::error(
                            "Backup wurde übersprungen – Details stehen im Protokoll.",
                        )
                        .ok();
                    }
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
                    serde_json::json!({ "backup_uuid": backup_uuid, "routine_uuid": data.routine_uuid }),
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

    mod routine {
        use crate::models::{self, Routine};
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{server::GetServer, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Deserialize)]
        pub struct Payload {
            routine_uuid: Option<uuid::Uuid>,
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
            let routines = Routine::all_for_server(&state.database, server.uuid).await?;
            let target = match data.routine_uuid {
                Some(uuid) => match routines.iter().find(|r| r.uuid == uuid) {
                    Some(routine) => Some(routine),
                    None => return ApiResponse::error("Routine nicht gefunden.").ok(),
                },
                None => None,
            };

            models::set_routine(&state.database, server.uuid, backup.uuid, target.map(|r| r.uuid))
                .await?;
            models::write_log(
                &state.database,
                server.uuid,
                "info",
                match target {
                    Some(routine) => format!(
                        "\"{}\" manuell der Routine \"{}\" zugeordnet",
                        backup.name, routine.name
                    ),
                    None => format!(
                        "\"{}\" aus der Rotation genommen (keiner Routine zugeordnet)",
                        backup.name
                    ),
                },
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
                "/routine",
                OpenApiRouter::new()
                    .routes(routes!(routine::route))
                    .with_state(state.clone()),
            )
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .nest("/apply-template", apply_template::router(state))
        .nest("/run", run::router(state))
        .nest("/backups/{backup}", backup::router(state))
        .with_state(state.clone())
}
