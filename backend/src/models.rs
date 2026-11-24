//! Datenbankzugriff für die extension-eigenen Tabellen.

use serde::{Deserialize, Serialize};
use shared::database::Database;
use sqlx::Row;
use utoipa::ToSchema;

/// Die Stufe (Tier) eines verwalteten Backups.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl Tier {
    pub const ALL: [Tier; 4] = [Tier::Hourly, Tier::Daily, Tier::Weekly, Tier::Monthly];

    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Hourly => "hourly",
            Tier::Daily => "daily",
            Tier::Weekly => "weekly",
            Tier::Monthly => "monthly",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tier::Hourly => "Stündlich",
            Tier::Daily => "Täglich",
            Tier::Weekly => "Wöchentlich",
            Tier::Monthly => "Monatlich",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "hourly" => Some(Tier::Hourly),
            "daily" => Some(Tier::Daily),
            "weekly" => Some(Tier::Weekly),
            "monthly" => Some(Tier::Monthly),
            _ => None,
        }
    }

    /// Mindestabstand zwischen zwei Backups dieser Stufe.
    pub fn period(self) -> chrono::Duration {
        match self {
            Tier::Hourly => chrono::Duration::hours(1),
            Tier::Daily => chrono::Duration::days(1),
            Tier::Weekly => chrono::Duration::days(7),
            Tier::Monthly => chrono::Duration::days(30),
        }
    }
}

/// Backup-Richtlinie eines Servers.
#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct Policy {
    pub server_uuid: uuid::Uuid,
    pub backup_group_uuid: Option<uuid::Uuid>,
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
    pub next_run: Option<chrono::NaiveDateTime>,
    pub last_run: Option<chrono::NaiveDateTime>,
    pub last_error: Option<String>,
}

const POLICY_COLUMNS: &str = "server_uuid, backup_group_uuid, enabled, interval_hours, minute_offset, \
    keep_hourly, keep_daily, keep_weekly, keep_monthly, name_template, ignored_files, delete_failed, \
    next_run, last_run, last_error";

impl Policy {
    /// Standard-Richtlinie: 3x stündlich, 1x wöchentlich, 1x monatlich.
    pub fn default_for(server_uuid: uuid::Uuid) -> Self {
        Self {
            server_uuid,
            backup_group_uuid: None,
            enabled: false,
            interval_hours: 1,
            minute_offset: 0,
            keep_hourly: 3,
            keep_daily: 0,
            keep_weekly: 1,
            keep_monthly: 1,
            name_template: "Auto {tier} {date}".into(),
            ignored_files: Vec::new(),
            delete_failed: true,
            next_run: None,
            last_run: None,
            last_error: None,
        }
    }

    pub fn keep(&self, tier: Tier) -> i32 {
        match tier {
            Tier::Hourly => self.keep_hourly,
            Tier::Daily => self.keep_daily,
            Tier::Weekly => self.keep_weekly,
            Tier::Monthly => self.keep_monthly,
        }
        .max(0)
    }

    pub fn keep_total(&self) -> i32 {
        Tier::ALL.iter().map(|t| self.keep(*t)).sum()
    }

    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            server_uuid: row.try_get("server_uuid")?,
            backup_group_uuid: row.try_get("backup_group_uuid")?,
            enabled: row.try_get("enabled")?,
            interval_hours: row.try_get("interval_hours")?,
            minute_offset: row.try_get("minute_offset")?,
            keep_hourly: row.try_get("keep_hourly")?,
            keep_daily: row.try_get("keep_daily")?,
            keep_weekly: row.try_get("keep_weekly")?,
            keep_monthly: row.try_get("keep_monthly")?,
            name_template: row.try_get("name_template")?,
            ignored_files: row.try_get("ignored_files")?,
            delete_failed: row.try_get("delete_failed")?,
            next_run: row.try_get("next_run")?,
            last_run: row.try_get("last_run")?,
            last_error: row.try_get("last_error")?,
        })
    }

    pub async fn by_server_uuid(
        database: &Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {POLICY_COLUMNS} FROM backup_plus_policies WHERE server_uuid = $1"
        )))
        .bind(server_uuid)
        .fetch_optional(database.write())
        .await?;

        row.map(|row| Self::from_row(&row)).transpose()
    }

    /// Alle aktiven Richtlinien, deren nächster Lauf fällig ist (älteste zuerst).
    pub async fn all_due(
        database: &Database,
        now: chrono::NaiveDateTime,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {POLICY_COLUMNS} FROM backup_plus_policies \
             WHERE enabled AND (next_run IS NULL OR next_run <= $1) \
             ORDER BY next_run NULLS FIRST"
        )))
        .bind(now)
        .fetch_all(database.write())
        .await?;

        rows.iter().map(Self::from_row).collect()
    }

    /// Alle aktiven Richtlinien (für die regelmäßige Bereinigung).
    pub async fn all_enabled(database: &Database) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {POLICY_COLUMNS} FROM backup_plus_policies WHERE enabled"
        )))
        .fetch_all(database.write())
        .await?;

        rows.iter().map(Self::from_row).collect()
    }

    pub async fn upsert(&self, database: &Database) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO backup_plus_policies (
                server_uuid, backup_group_uuid, enabled, interval_hours, minute_offset,
                keep_hourly, keep_daily, keep_weekly, keep_monthly, name_template,
                ignored_files, delete_failed, next_run, last_run, last_error, updated
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, NOW() AT TIME ZONE 'UTC')
            ON CONFLICT (server_uuid) DO UPDATE SET
                backup_group_uuid = EXCLUDED.backup_group_uuid,
                enabled = EXCLUDED.enabled,
                interval_hours = EXCLUDED.interval_hours,
                minute_offset = EXCLUDED.minute_offset,
                keep_hourly = EXCLUDED.keep_hourly,
                keep_daily = EXCLUDED.keep_daily,
                keep_weekly = EXCLUDED.keep_weekly,
                keep_monthly = EXCLUDED.keep_monthly,
                name_template = EXCLUDED.name_template,
                ignored_files = EXCLUDED.ignored_files,
                delete_failed = EXCLUDED.delete_failed,
                next_run = EXCLUDED.next_run,
                last_run = EXCLUDED.last_run,
                last_error = EXCLUDED.last_error,
                updated = EXCLUDED.updated
            "#,
        )
        .bind(self.server_uuid)
        .bind(self.backup_group_uuid)
        .bind(self.enabled)
        .bind(self.interval_hours)
        .bind(self.minute_offset)
        .bind(self.keep_hourly)
        .bind(self.keep_daily)
        .bind(self.keep_weekly)
        .bind(self.keep_monthly)
        .bind(&self.name_template)
        .bind(&self.ignored_files)
        .bind(self.delete_failed)
        .bind(self.next_run)
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
            UPDATE backup_plus_policies
            SET backup_group_uuid = $2, next_run = $3, last_run = $4, last_error = $5
            WHERE server_uuid = $1
            "#,
        )
        .bind(self.server_uuid)
        .bind(self.backup_group_uuid)
        .bind(self.next_run)
        .bind(self.last_run)
        .bind(&self.last_error)
        .execute(database.write())
        .await?;

        Ok(())
    }
}

/// Ein Backup aus der verwalteten Backup-Gruppe, inklusive Stufe.
#[derive(Clone, Debug)]
pub struct ManagedBackup {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub tier: Tier,
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
                backup_plus_backups.tier
            FROM server_backups
            LEFT JOIN backup_plus_backups ON backup_plus_backups.backup_uuid = server_backups.uuid
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
                let tier: Option<String> = row.try_get("tier")?;
                let name: compact_str::CompactString = row.try_get("name")?;

                Ok(Self {
                    uuid: row.try_get("uuid")?,
                    name: name.to_string(),
                    tier: tier.as_deref().and_then(Tier::parse).unwrap_or(Tier::Hourly),
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

pub async fn set_tier(
    database: &Database,
    server_uuid: uuid::Uuid,
    backup_uuid: uuid::Uuid,
    tier: Tier,
    promoted: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO backup_plus_backups (backup_uuid, server_uuid, tier, promoted)
        VALUES ($1, $2, $3, CASE WHEN $4 THEN NOW() AT TIME ZONE 'UTC' ELSE NULL END)
        ON CONFLICT (backup_uuid) DO UPDATE SET
            tier = EXCLUDED.tier,
            promoted = COALESCE(EXCLUDED.promoted, backup_plus_backups.promoted)
        "#,
    )
    .bind(backup_uuid)
    .bind(server_uuid)
    .bind(tier.as_str())
    .bind(promoted)
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
    message: impl AsRef<str>,
) {
    let result = sqlx::query(
        "INSERT INTO backup_plus_log (server_uuid, level, message) VALUES ($1, $2, $3)",
    )
    .bind(server_uuid)
    .bind(level)
    .bind(message.as_ref())
    .execute(database.write())
    .await;

    if let Err(err) = result {
        tracing::warn!(server = %server_uuid, "backup-plus: Log-Eintrag fehlgeschlagen: {err:#?}");
    }
}

pub async fn read_log(
    database: &Database,
    server_uuid: uuid::Uuid,
    limit: i64,
) -> Result<Vec<LogEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT level, message, created
        FROM backup_plus_log
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
                message: row.try_get("message")?,
                created: created.and_utc(),
            })
        })
        .collect()
}

/// Löscht alte Protokolleinträge, sodass pro Server höchstens `keep` übrig bleiben.
pub async fn trim_log(database: &Database, keep: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM backup_plus_log
        WHERE id IN (
            SELECT id FROM (
                SELECT id, ROW_NUMBER() OVER (PARTITION BY server_uuid ORDER BY created DESC, id DESC) AS rn
                FROM backup_plus_log
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

#[derive(ToSchema, Serialize)]
pub struct Overview {
    pub policies_total: i64,
    pub policies_enabled: i64,
    pub managed_backups: i64,
    pub managed_bytes: i64,
    pub policies_with_errors: i64,
}

pub async fn overview(database: &Database) -> Result<Overview, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*) FROM backup_plus_policies) AS policies_total,
            (SELECT COUNT(*) FROM backup_plus_policies WHERE enabled) AS policies_enabled,
            (SELECT COUNT(*) FROM backup_plus_policies WHERE enabled AND last_error IS NOT NULL) AS policies_with_errors,
            (SELECT COUNT(*) FROM server_backups
                JOIN backup_plus_policies ON backup_plus_policies.backup_group_uuid = server_backups.backup_group_uuid
                WHERE server_backups.deleted IS NULL) AS managed_backups,
            (SELECT COALESCE(SUM(server_backups.bytes), 0)::BIGINT FROM server_backups
                JOIN backup_plus_policies ON backup_plus_policies.backup_group_uuid = server_backups.backup_group_uuid
                WHERE server_backups.deleted IS NULL) AS managed_bytes
        "#,
    )
    .fetch_one(database.write())
    .await?;

    Ok(Overview {
        policies_total: row.try_get("policies_total")?,
        policies_enabled: row.try_get("policies_enabled")?,
        managed_backups: row.try_get("managed_backups")?,
        managed_bytes: row.try_get("managed_bytes")?,
        policies_with_errors: row.try_get("policies_with_errors")?,
    })
}
