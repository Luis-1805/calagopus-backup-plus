//! Datenbankzugriff für die extension-eigenen Tabellen.

use serde::{Deserialize, Serialize};
use shared::database::Database;
use sqlx::Row;
use utoipa::ToSchema;

/// Server-weite Einstellungen von Backups Plus.
#[derive(Clone)]
pub struct Policy {
    pub server_uuid: uuid::Uuid,
    pub backup_group_uuid: Option<uuid::Uuid>,
    pub enabled: bool,
    pub delete_failed: bool,
    pub last_run: Option<chrono::NaiveDateTime>,
    pub last_error: Option<String>,
}

impl Policy {
    pub fn default_for(server_uuid: uuid::Uuid) -> Self {
        Self {
            server_uuid,
            backup_group_uuid: None,
            enabled: false,
            delete_failed: true,
            last_run: None,
            last_error: None,
        }
    }

    pub async fn by_server_uuid(
        database: &Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT server_uuid, backup_group_uuid, enabled, delete_failed, last_run, last_error
            FROM backups_plus_policies
            WHERE server_uuid = $1
            "#,
        )
        .bind(server_uuid)
        .fetch_optional(database.write())
        .await?;

        row.map(|row| {
            Ok(Self {
                server_uuid: row.try_get("server_uuid")?,
                backup_group_uuid: row.try_get("backup_group_uuid")?,
                enabled: row.try_get("enabled")?,
                delete_failed: row.try_get("delete_failed")?,
                last_run: row.try_get("last_run")?,
                last_error: row.try_get("last_error")?,
            })
        })
        .transpose()
    }

    pub async fn upsert(&self, database: &Database) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO backups_plus_policies (
                server_uuid, backup_group_uuid, enabled, delete_failed, last_run, last_error, updated
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW() AT TIME ZONE 'UTC')
            ON CONFLICT (server_uuid) DO UPDATE SET
                backup_group_uuid = EXCLUDED.backup_group_uuid,
                enabled = EXCLUDED.enabled,
                delete_failed = EXCLUDED.delete_failed,
                last_run = EXCLUDED.last_run,
                last_error = EXCLUDED.last_error,
                updated = EXCLUDED.updated
            "#,
        )
        .bind(self.server_uuid)
        .bind(self.backup_group_uuid)
        .bind(self.enabled)
        .bind(self.delete_failed)
        .bind(self.last_run)
        .bind(&self.last_error)
        .execute(database.write())
        .await?;

        Ok(())
    }

    /// Aktualisiert nur die Laufzeit-Felder (ohne vom Nutzer geänderte Einstellungen zu überschreiben).
    pub async fn save_run_state(&self, database: &Database) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE backups_plus_policies
            SET backup_group_uuid = $2, last_run = $3, last_error = $4
            WHERE server_uuid = $1
            "#,
        )
        .bind(self.server_uuid)
        .bind(self.backup_group_uuid)
        .bind(self.last_run)
        .bind(&self.last_error)
        .execute(database.write())
        .await?;

        Ok(())
    }

    /// Server mit aktiver Richtlinie, bei denen mindestens eine aktive Routine fällig ist.
    pub async fn servers_due(
        database: &Database,
        now: chrono::NaiveDateTime,
    ) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
        sqlx::query_scalar(
            r#"
            SELECT backups_plus_policies.server_uuid
            FROM backups_plus_policies
            WHERE backups_plus_policies.enabled
                AND EXISTS (
                    SELECT 1 FROM backups_plus_routines
                    WHERE backups_plus_routines.server_uuid = backups_plus_policies.server_uuid
                        AND backups_plus_routines.enabled
                        AND backups_plus_routines.source = 'create'
                        AND (backups_plus_routines.next_run IS NULL OR backups_plus_routines.next_run <= $1)
                )
            ORDER BY backups_plus_policies.last_run NULLS FIRST
            "#,
        )
        .bind(now)
        .fetch_all(database.write())
        .await
    }

    /// Alle Server mit aktiver Richtlinie.
    pub async fn servers_enabled(database: &Database) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
        sqlx::query_scalar("SELECT server_uuid FROM backups_plus_policies WHERE enabled")
            .fetch_all(database.write())
            .await
    }
}

/// Woher eine Routine ihre Backups bekommt.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutineSource {
    /// Erstellt zum Zeitplan ein neues Backup.
    Create,
    /// Übernimmt zum Zeitplan ein Backup, das aus einer anderen Routine herausfällt.
    Promote,
}

impl RoutineSource {
    pub fn as_str(self) -> &'static str {
        match self {
            RoutineSource::Create => "create",
            RoutineSource::Promote => "promote",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "promote" => RoutineSource::Promote,
            _ => RoutineSource::Create,
        }
    }
}

/// Eine Backup-Routine eines Servers.
#[derive(Clone, Debug)]
pub struct Routine {
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub name: String,
    pub enabled: bool,
    pub position: i32,
    pub cron: String,
    pub source: RoutineSource,
    pub source_routine_uuid: Option<uuid::Uuid>,
    pub keep_count: i32,
    pub max_age_hours: i32,
    pub name_template: String,
    /// Optional: neuer Name für Backups, die diese Routine übernimmt.
    pub rename_template: Option<String>,
    pub ignored_files: Vec<String>,
    pub next_run: Option<chrono::NaiveDateTime>,
    pub last_run: Option<chrono::NaiveDateTime>,
    pub last_error: Option<String>,
}

impl Routine {
    pub fn is_due(&self, now: chrono::NaiveDateTime) -> bool {
        self.enabled && self.next_run.is_none_or(|next| next <= now)
    }

    pub async fn all_for_server(
        database: &Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT uuid, server_uuid, name, enabled, position, cron, source, source_routine_uuid,
                keep_count, max_age_hours, name_template, rename_template, ignored_files, next_run, last_run, last_error
            FROM backups_plus_routines
            WHERE server_uuid = $1
            ORDER BY position, created, uuid
            "#,
        )
        .bind(server_uuid)
        .fetch_all(database.write())
        .await?;

        rows.iter()
            .map(|row| {
                let source: String = row.try_get("source")?;
                Ok(Self {
                    uuid: row.try_get("uuid")?,
                    server_uuid: row.try_get("server_uuid")?,
                    name: row.try_get("name")?,
                    enabled: row.try_get("enabled")?,
                    position: row.try_get("position")?,
                    cron: row.try_get("cron")?,
                    source: RoutineSource::parse(&source),
                    source_routine_uuid: row.try_get("source_routine_uuid")?,
                    keep_count: row.try_get("keep_count")?,
                    max_age_hours: row.try_get("max_age_hours")?,
                    name_template: row.try_get("name_template")?,
                    rename_template: row.try_get("rename_template")?,
                    ignored_files: row.try_get("ignored_files")?,
                    next_run: row.try_get("next_run")?,
                    last_run: row.try_get("last_run")?,
                    last_error: row.try_get("last_error")?,
                })
            })
            .collect()
    }

    pub async fn save_run_state(&self, database: &Database) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE backups_plus_routines
            SET next_run = $2, last_run = $3, last_error = $4
            WHERE uuid = $1
            "#,
        )
        .bind(self.uuid)
        .bind(self.next_run)
        .bind(self.last_run)
        .bind(&self.last_error)
        .execute(database.write())
        .await?;

        Ok(())
    }

    /// Ersetzt alle Routinen eines Servers in einer Transaktion.
    ///
    /// Routinen mit bekannter UUID werden aktualisiert (Laufzeitstatus bleibt erhalten),
    /// neue eingefügt und nicht mehr enthaltene gelöscht. Deren Backups werden dabei
    /// nicht gelöscht, sondern verlieren nur ihre Zuordnung.
    pub async fn replace_all(
        database: &Database,
        server_uuid: uuid::Uuid,
        routines: &[Routine],
    ) -> Result<(), sqlx::Error> {
        if routines.iter().any(|r| r.server_uuid != server_uuid) {
            return Err(sqlx::Error::Protocol(
                "routine belongs to another server".into(),
            ));
        }

        let mut transaction = database.write().begin().await?;

        let keep: Vec<uuid::Uuid> = routines.iter().map(|r| r.uuid).collect();
        sqlx::query("DELETE FROM backups_plus_routines WHERE server_uuid = $1 AND NOT (uuid = ANY($2))")
            .bind(server_uuid)
            .bind(&keep)
            .execute(&mut *transaction)
            .await?;

        // Erst ohne Quellverweis speichern, damit die Fremdschlüssel in beliebiger Reihenfolge passen.
        for routine in routines {
            sqlx::query(
                r#"
                INSERT INTO backups_plus_routines (
                    uuid, server_uuid, name, enabled, position, cron, source, source_routine_uuid,
                    keep_count, max_age_hours, name_template, ignored_files, next_run, rename_template
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, $10, $11, $12, $13)
                ON CONFLICT (uuid) DO UPDATE SET
                    name = EXCLUDED.name,
                    enabled = EXCLUDED.enabled,
                    position = EXCLUDED.position,
                    cron = EXCLUDED.cron,
                    source = EXCLUDED.source,
                    source_routine_uuid = NULL,
                    keep_count = EXCLUDED.keep_count,
                    max_age_hours = EXCLUDED.max_age_hours,
                    name_template = EXCLUDED.name_template,
                    rename_template = EXCLUDED.rename_template,
                    ignored_files = EXCLUDED.ignored_files,
                    next_run = EXCLUDED.next_run
                "#,
            )
            .bind(routine.uuid)
            .bind(server_uuid)
            .bind(&routine.name)
            .bind(routine.enabled)
            .bind(routine.position)
            .bind(&routine.cron)
            .bind(routine.source.as_str())
            .bind(routine.keep_count)
            .bind(routine.max_age_hours)
            .bind(&routine.name_template)
            .bind(&routine.ignored_files)
            .bind(routine.next_run)
            .bind(routine.rename_template.as_deref().map(str::trim).filter(|t| !t.is_empty()))
            .execute(&mut *transaction)
            .await?;
        }

        for routine in routines {
            if let Some(source) = routine.source_routine_uuid {
                sqlx::query("UPDATE backups_plus_routines SET source_routine_uuid = $2 WHERE uuid = $1")
                    .bind(routine.uuid)
                    .bind(source)
                    .execute(&mut *transaction)
                    .await?;
            }
        }

        transaction.commit().await?;

        Ok(())
    }
}

/// Ein Backup aus der verwalteten Backup-Gruppe, inklusive Routinen-Zuordnung.
#[derive(Clone, Debug)]
pub struct ManagedBackup {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub routine_uuid: Option<uuid::Uuid>,
    pub successful: bool,
    pub locked: bool,
    pub bytes: i64,
    pub completed: Option<chrono::NaiveDateTime>,
    pub created: chrono::NaiveDateTime,
}

impl ManagedBackup {
    /// Alle nicht gelöschten Backups der Gruppe (neueste zuerst).
    pub async fn all_for_group(
        database: &Database,
        server_uuid: uuid::Uuid,
        backup_group_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                server_backups.uuid,
                server_backups.name,
                server_backups.successful,
                server_backups.locked,
                server_backups.bytes,
                server_backups.completed,
                server_backups.created,
                backups_plus_backups.routine_uuid
            FROM server_backups
            LEFT JOIN backups_plus_backups ON backups_plus_backups.backup_uuid = server_backups.uuid
            WHERE
                server_backups.server_uuid = $1
                AND server_backups.backup_group_uuid = $2
                AND server_backups.deleted IS NULL
                AND server_backups.deleting IS NULL
            ORDER BY server_backups.created DESC, server_backups.uuid DESC
            "#,
        )
        .bind(server_uuid)
        .bind(backup_group_uuid)
        .fetch_all(database.write())
        .await?;

        rows.iter()
            .map(|row| {
                let name: compact_str::CompactString = row.try_get("name")?;

                Ok(Self {
                    uuid: row.try_get("uuid")?,
                    name: name.to_string(),
                    routine_uuid: row.try_get("routine_uuid")?,
                    successful: row.try_get("successful")?,
                    locked: row.try_get("locked")?,
                    bytes: row.try_get("bytes")?,
                    completed: row.try_get("completed")?,
                    created: row.try_get("created")?,
                })
            })
            .collect()
    }

    /// Anzahl der Backups der Gruppe, die gerade noch laufen.
    pub async fn count_running(
        database: &Database,
        backup_group_uuid: uuid::Uuid,
        started_after: chrono::NaiveDateTime,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM server_backups
            WHERE
                server_backups.backup_group_uuid = $1
                AND server_backups.deleted IS NULL
                AND server_backups.deleting IS NULL
                AND server_backups.completed IS NULL
                AND server_backups.created > $2
            "#,
        )
        .bind(backup_group_uuid)
        .bind(started_after)
        .fetch_one(database.write())
        .await
    }
}

pub async fn set_routine(
    database: &Database,
    server_uuid: uuid::Uuid,
    backup_uuid: uuid::Uuid,
    routine_uuid: Option<uuid::Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO backups_plus_backups (backup_uuid, server_uuid, routine_uuid, promoted)
        VALUES ($1, $2, $3, NOW() AT TIME ZONE 'UTC')
        ON CONFLICT (backup_uuid) DO UPDATE SET
            routine_uuid = EXCLUDED.routine_uuid,
            promoted = EXCLUDED.promoted
        "#,
    )
    .bind(backup_uuid)
    .bind(server_uuid)
    .bind(routine_uuid)
    .execute(database.write())
    .await?;

    Ok(())
}

#[derive(ToSchema, Serialize, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

pub async fn write_log(
    database: &Database,
    server_uuid: uuid::Uuid,
    level: &str,
    message: crate::i18n::Msg,
) {
    let result = sqlx::query(
        "INSERT INTO backups_plus_log (server_uuid, level, message) VALUES ($1, $2, $3)",
    )
    .bind(server_uuid)
    .bind(level)
    .bind(message.to_json())
    .execute(database.write())
    .await;

    if let Err(err) = result {
        tracing::warn!(server = %server_uuid, "backups-plus: failed to write log entry: {err:#?}");
    }
}

pub async fn read_log(
    database: &Database,
    server_uuid: uuid::Uuid,
    limit: i64,
    lang: crate::i18n::Lang,
) -> Result<Vec<LogEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT level, message, created
        FROM backups_plus_log
        WHERE server_uuid = $1
        ORDER BY created DESC, id DESC
        LIMIT $2
        "#,
    )
    .bind(server_uuid)
    .bind(limit)
    .fetch_all(database.write())
    .await?;

    rows.iter()
        .map(|row| {
            let created: chrono::NaiveDateTime = row.try_get("created")?;
            Ok(LogEntry {
                level: row.try_get("level")?,
                message: crate::i18n::Msg::from_stored(&row.try_get::<String, _>("message")?)
                    .render(lang),
                created: created.and_utc(),
            })
        })
        .collect()
}

/// Löscht alte Protokolleinträge, sodass pro Server höchstens `keep` übrig bleiben.
pub async fn trim_log(database: &Database, keep: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM backups_plus_log
        WHERE id IN (
            SELECT id FROM (
                SELECT id, ROW_NUMBER() OVER (PARTITION BY server_uuid ORDER BY created DESC, id DESC) AS rn
                FROM backups_plus_log
            ) ranked
            WHERE ranked.rn > $1
        )
        "#,
    )
    .bind(keep)
    .execute(database.write())
    .await?;

    Ok(())
}

/// UUIDs aller Server (für das Anwenden einer Vorlage auf alle Server).
pub async fn all_server_uuids(
    database: &Database,
    only_unconfigured: bool,
) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT servers.uuid
        FROM servers
        WHERE NOT $1 OR NOT EXISTS (
            SELECT 1 FROM backups_plus_routines WHERE backups_plus_routines.server_uuid = servers.uuid
        )
        ORDER BY servers.created
        "#,
    )
    .bind(only_unconfigured)
    .fetch_all(database.write())
    .await
}

#[derive(ToSchema, Serialize)]
pub struct Overview {
    pub policies_total: i64,
    pub policies_enabled: i64,
    pub routines_total: i64,
    pub managed_backups: i64,
    pub managed_bytes: i64,
    pub policies_with_errors: i64,
}

pub async fn overview(database: &Database) -> Result<Overview, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*) FROM backups_plus_policies) AS policies_total,
            (SELECT COUNT(*) FROM backups_plus_policies WHERE enabled) AS policies_enabled,
            (SELECT COUNT(*) FROM backups_plus_routines) AS routines_total,
            (SELECT COUNT(DISTINCT backups_plus_routines.server_uuid) FROM backups_plus_routines
                JOIN backups_plus_policies ON backups_plus_policies.server_uuid = backups_plus_routines.server_uuid
                WHERE backups_plus_policies.enabled AND backups_plus_routines.last_error IS NOT NULL) AS policies_with_errors,
            (SELECT COUNT(*) FROM server_backups
                JOIN backups_plus_policies ON backups_plus_policies.backup_group_uuid = server_backups.backup_group_uuid
                WHERE server_backups.deleted IS NULL) AS managed_backups,
            (SELECT COALESCE(SUM(server_backups.bytes), 0)::BIGINT FROM server_backups
                JOIN backups_plus_policies ON backups_plus_policies.backup_group_uuid = server_backups.backup_group_uuid
                WHERE server_backups.deleted IS NULL) AS managed_bytes
        "#,
    )
    .fetch_one(database.write())
    .await?;

    Ok(Overview {
        policies_total: row.try_get("policies_total")?,
        policies_enabled: row.try_get("policies_enabled")?,
        routines_total: row.try_get("routines_total")?,
        managed_backups: row.try_get("managed_backups")?,
        managed_bytes: row.try_get("managed_bytes")?,
        policies_with_errors: row.try_get("policies_with_errors")?,
    })
}
