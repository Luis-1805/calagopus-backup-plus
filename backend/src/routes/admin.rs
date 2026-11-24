//! Routen unter `/api/admin/extensions/dev.backupplus.gfsbackups`.

use shared::State;
use utoipa_axum::router::OpenApiRouter;

mod settings {
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
            settings: crate::settings::ExtensionSettingsData,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
            permissions.has_admin_permission("extensions.read")?;

            ApiResponse::new_serialized(Response {
                settings: crate::settings::load(&state).await,
            })
            .ok()
        }
    }

    mod put {
        use axum::http::StatusCode;
        use garde::Validate;
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(skip)]
            enabled: bool,
            #[garde(range(min = 1, max = 168))]
            #[schema(minimum = 1, maximum = 168)]
            min_interval_hours: u32,
            #[garde(range(min = 1, max = 500))]
            #[schema(minimum = 1, maximum = 500)]
            max_keep_total: u32,
            #[garde(range(min = 1, max = 1000))]
            #[schema(minimum = 1, maximum = 1000)]
            max_backups_per_tick: u32,
            #[garde(range(min = 10, max = 10000))]
            #[schema(minimum = 10, maximum = 10000)]
            log_retention: u32,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {}

        #[utoipa::path(put, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            activity_logger: GetAdminActivityLogger,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("extensions.manage")?;

            {
                let mut settings = state.settings.get_mut().await?;
                let ext: &mut crate::settings::ExtensionSettingsData =
                    settings.find_mut_extension_settings()?;

                ext.enabled = data.enabled;
                ext.min_interval_hours = data.min_interval_hours;
                ext.max_keep_total = data.max_keep_total;
                ext.max_backups_per_tick = data.max_backups_per_tick;
                ext.log_retention = data.log_retention;

                settings.save().await?;
            }

            activity_logger
                .log(
                    "backup-plus:settings.update",
                    serde_json::json!({
                        "enabled": data.enabled,
                        "min_interval_hours": data.min_interval_hours,
                        "max_keep_total": data.max_keep_total,
                        "max_backups_per_tick": data.max_backups_per_tick,
                        "log_retention": data.log_retention,
                    }),
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
        pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
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
        .nest("/overview", overview::router(state))
        .with_state(state.clone())
}
