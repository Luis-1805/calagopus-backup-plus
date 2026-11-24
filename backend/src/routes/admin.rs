//! Routen unter `/api/admin/extensions/dev.backupsplus.backups`.

use shared::State;
use utoipa_axum::router::OpenApiRouter;

mod settings {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize, garde::Validate)]
    pub struct GeneralSettings {
        #[garde(skip)]
        enabled: bool,
        #[garde(range(min = 5, max = 604800))]
        #[schema(minimum = 5, maximum = 604800)]
        min_interval_seconds: u32,
        #[garde(range(min = 1, max = 500))]
        #[schema(minimum = 1, maximum = 500)]
        max_keep_total: u32,
        #[garde(range(min = 1, max = 50))]
        #[schema(minimum = 1, maximum = 50)]
        max_routines: u32,
        #[garde(skip)]
        allow_custom_routines: bool,
        #[garde(length(chars, max = 64))]
        default_template_id: String,
        #[garde(length(chars, max = 64))]
        #[serde(default)]
        default_timezone: String,
        #[garde(range(min = 1, max = 1000))]
        #[schema(minimum = 1, maximum = 1000)]
        max_backups_per_tick: u32,
        #[garde(range(min = 10, max = 10000))]
        #[schema(minimum = 10, maximum = 10000)]
        log_retention: u32,
    }

    mod get {
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::user::GetPermissionManager,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            settings: super::GeneralSettings,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("extensions.read")?;

            let s = crate::settings::load(&state).await;
            ApiResponse::new_serialized(Response {
                settings: super::GeneralSettings {
                    enabled: s.enabled,
                    min_interval_seconds: s.min_interval_seconds,
                    max_keep_total: s.max_keep_total,
                    max_routines: s.max_routines,
                    allow_custom_routines: s.allow_custom_routines,
                    default_template_id: s.default_template_id,
                    default_timezone: s.default_timezone,
                    max_backups_per_tick: s.max_backups_per_tick,
                    log_retention: s.log_retention,
                },
            })
            .ok()
        }
    }

    mod put {
        use axum::http::StatusCode;
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {}

        #[utoipa::path(put, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(super::GeneralSettings))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: shared::models::user::GetUser,
            activity_logger: GetAdminActivityLogger,
            shared::Payload(data): shared::Payload<super::GeneralSettings>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("extensions.manage")?;
            let lang = crate::i18n::Lang::from_code(&user.language);

            let timezone = data.default_timezone.trim();
            if !timezone.is_empty() && timezone.parse::<chrono_tz::Tz>().is_err() {
                return ApiResponse::error(
                    crate::i18n::Msg::new("err.invalid_timezone")
                        .arg("timezone", timezone)
                        .render(lang),
                )
                .ok();
            }

            let interval_lowered;
            {
                let mut settings = state.settings.get_mut().await?;
                let ext: &mut crate::settings::ExtensionSettingsData =
                    settings.find_mut_extension_settings()?;

                if !data.default_template_id.is_empty()
                    && ext.find_template(&data.default_template_id, lang).is_none()
                {
                    return ApiResponse::error(crate::i18n::t("err.default_template_missing", lang)).ok();
                }

                ext.enabled = data.enabled;
                interval_lowered = data.min_interval_seconds < ext.min_interval_seconds;
                ext.min_interval_seconds = data.min_interval_seconds;
                ext.max_keep_total = data.max_keep_total;
                ext.max_routines = data.max_routines;
                ext.allow_custom_routines = data.allow_custom_routines;
                ext.default_template_id = data.default_template_id.clone();
                ext.default_timezone = data.default_timezone.trim().to_string();
                ext.max_backups_per_tick = data.max_backups_per_tick;
                ext.log_retention = data.log_retention;

                settings.save().await?;
            }

            // Kleinerer Abstand: zurückgehaltene Termine sofort vorziehen.
            if interval_lowered {
                for server_uuid in crate::models::Policy::servers_enabled(&state.database).await? {
                    if let Err(err) = crate::engine::reschedule(&state, server_uuid).await {
                        tracing::warn!(server = %server_uuid, "backups-plus: reschedule failed: {err:#?}");
                    }
                }
            }

            activity_logger
                .log(
                    "backups-plus:settings.update",
                    serde_json::to_value(&data).unwrap_or_default(),
                )
                .await;

            ApiResponse::new_serialized(Response {}).ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(get::route))
            .routes(routes!(put::route))
            .with_state(state.clone())
    }
}

mod templates {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod get {
        use crate::templates::Template;
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::user::GetPermissionManager,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            templates: Vec<Template>,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: shared::models::user::GetUser,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("extensions.read")?;
            let lang = crate::i18n::Lang::from_code(&user.language);

            ApiResponse::new_serialized(Response {
                templates: crate::settings::load(&state).await.all_templates(lang),
            })
            .ok()
        }
    }

    mod put {
        use crate::{templates::{self, Template}, validation};
        use axum::http::StatusCode;
        use garde::Validate;
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use std::collections::HashSet;
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            /// Nur eigene Vorlagen; eingebaute werden ignoriert.
            #[garde(length(max = 50), dive)]
            #[schema(max_items = 50)]
            templates: Vec<Template>,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            templates: Vec<Template>,
        }

        #[utoipa::path(put, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: shared::models::user::GetUser,
            activity_logger: GetAdminActivityLogger,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("extensions.manage")?;
            let lang = crate::i18n::Lang::from_code(&user.language);

            let builtin_ids: HashSet<String> =
                templates::builtin(lang).into_iter().map(|t| t.id).collect();
            let custom: Vec<Template> = data
                .templates
                .into_iter()
                .filter(|t| !t.builtin && !builtin_ids.contains(&t.id))
                .collect();

            let mut ids = HashSet::new();
            let settings = crate::settings::load(&state).await;
            for template in &custom {
                if !ids.insert(template.id.clone()) {
                    return ApiResponse::error(
                        crate::i18n::Msg::new("err.duplicate_template_id")
                            .arg("id", &template.id)
                            .render(lang),
                    )
                    .ok();
                }

                let routines = match templates::to_routines(template, uuid::Uuid::nil()) {
                    Ok(routines) => routines,
                    Err(err) => {
                        return ApiResponse::error(
                            crate::i18n::Msg::new("err.template_invalid")
                                .arg("template", &template.name)
                                .arg_msg("error", &err)
                                .render(lang),
                        )
                        .ok();
                    }
                };
                if let Err(err) = validation::validate(&routines, &settings, None) {
                    return ApiResponse::error(
                            crate::i18n::Msg::new("err.template_invalid")
                                .arg("template", &template.name)
                                .arg_msg("error", &err)
                                .render(lang),
                        )
                        .ok();
                }
            }

            {
                let mut settings = state.settings.get_mut().await?;
                let ext: &mut crate::settings::ExtensionSettingsData =
                    settings.find_mut_extension_settings()?;
                ext.templates = custom.clone();
                if ext.find_template(&ext.default_template_id.clone(), lang).is_none() {
                    ext.default_template_id = templates::DEFAULT_TEMPLATE_ID.into();
                }
                settings.save().await?;
            }

            activity_logger
                .log(
                    "backups-plus:templates.update",
                    serde_json::json!({ "templates": custom.iter().map(|t| &t.id).collect::<Vec<_>>() }),
                )
                .await;

            ApiResponse::new_serialized(Response {
                templates: templates::all(&custom, lang),
            })
            .ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(get::route))
            .routes(routes!(put::route))
            .with_state(state.clone())
    }
}

mod apply_template {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod post {
        use crate::{
            models::{self, Policy, Routine},
            schedule, templates,
            validation::{self, ServerLimits},
        };
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{
                ByUuid, admin_activity::GetAdminActivityLogger, server::Server,
                user::GetPermissionManager,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Deserialize)]
        pub struct Payload {
            template_id: String,
            /// Nur Server ohne eigene Routinen.
            #[serde(default = "default_true")]
            only_unconfigured: bool,
            /// Automatische Backups gleich aktivieren.
            #[serde(default)]
            enable: bool,
        }

        fn default_true() -> bool {
            true
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            applied: u32,
            skipped: Vec<String>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: shared::models::user::GetUser,
            activity_logger: GetAdminActivityLogger,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("extensions.manage")?;
            let lang = crate::i18n::Lang::from_code(&user.language);

            let settings = crate::settings::load(&state).await;
            let Some(template) = settings.find_template(&data.template_id, lang) else {
                return ApiResponse::error(crate::i18n::t("err.template_not_found", lang)).ok();
            };

            let mut applied = 0;
            let mut skipped = Vec::new();
            for server_uuid in models::all_server_uuids(&state.database, data.only_unconfigured).await? {
                let server = match Server::by_uuid(&state.database, server_uuid).await {
                    Ok(server) => server,
                    Err(_) => continue,
                };

                let routines = match templates::to_routines(&template, server.uuid) {
                    Ok(routines) => routines,
                    Err(err) => return ApiResponse::error(err.render(lang)).ok(),
                };
                let mut policy = Policy::by_server_uuid(&state.database, server.uuid)
                    .await?
                    .unwrap_or_else(|| Policy::default_for(server.uuid));
                if let Err(err) = validation::validate(
                    &routines,
                    &settings,
                    Some(&ServerLimits {
                        backup_limit: server.backup_limit,
                        timezone: schedule::resolve_timezone(&state, &server, &settings.default_timezone)
                            .await
                            .0,
                        min_interval_seconds: settings.min_interval_for(policy.min_interval_seconds),
                    }),
                ) {
                    skipped.push(format!("{}: {}", server.name, err.render(lang)));
                    continue;
                }

                if data.enable {
                    policy.enabled = true;
                }
                policy.last_error = None;
                policy.upsert(&state.database).await?;
                Routine::replace_all(&state.database, server.uuid, &routines).await?;
                models::write_log(
                    &state.database,
                    server.uuid,
                    "info",
                    crate::i18n::Msg::new("log.template_applied_admin").arg("template", &template.name),
                )
                .await;
                applied += 1;
            }

            activity_logger
                .log(
                    "backups-plus:templates.apply",
                    serde_json::json!({
                        "template": template.id,
                        "applied": applied,
                        "skipped": skipped.len(),
                        "enable": data.enable,
                    }),
                )
                .await;

            ApiResponse::new_serialized(Response { applied, skipped }).ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(post::route))
            .with_state(state.clone())
    }
}

mod overview {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod get {
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::user::GetPermissionManager,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            #[schema(inline)]
            overview: crate::models::Overview,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("extensions.read")?;

            ApiResponse::new_serialized(Response {
                overview: crate::models::overview(&state.database).await?,
            })
            .ok()
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(get::route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/settings", settings::router(state))
        .nest("/templates", templates::router(state))
        .nest("/apply-template", apply_template::router(state))
        .nest("/overview", overview::router(state))
        .with_state(state.clone())
}
