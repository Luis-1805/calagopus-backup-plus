-- Zurück auf feste Stufen (v1). Routinen-Konfiguration geht dabei verloren,
-- es werden die Standardwerte von v1 gesetzt.

ALTER TABLE backup_plus_policies
    ADD COLUMN IF NOT EXISTS interval_hours INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS minute_offset INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS keep_hourly INT NOT NULL DEFAULT 3,
    ADD COLUMN IF NOT EXISTS keep_daily INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS keep_weekly INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS keep_monthly INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS name_template VARCHAR(255) NOT NULL DEFAULT 'Auto {date}',
    ADD COLUMN IF NOT EXISTS ignored_files TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS next_run TIMESTAMP;

CREATE INDEX IF NOT EXISTS backup_plus_policies_enabled_next_run_idx
    ON backup_plus_policies (enabled, next_run);

ALTER TABLE backup_plus_backups
    ADD COLUMN IF NOT EXISTS tier VARCHAR(16) NOT NULL DEFAULT 'hourly';

ALTER TABLE backup_plus_backups DROP COLUMN IF EXISTS routine_uuid;

DROP TABLE IF EXISTS backup_plus_routines;
