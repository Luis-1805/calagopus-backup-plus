-- Frei konfigurierbare Backup-Routinen statt fester Stufen.

CREATE TABLE IF NOT EXISTS backup_plus_routines (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_uuid UUID NOT NULL REFERENCES servers(uuid) ON DELETE CASCADE,
    name VARCHAR(64) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    position INT NOT NULL DEFAULT 0,
    cron VARCHAR(128) NOT NULL,
    source VARCHAR(16) NOT NULL DEFAULT 'create',
    source_routine_uuid UUID REFERENCES backup_plus_routines(uuid) ON DELETE SET NULL,
    keep_count INT NOT NULL DEFAULT 1,
    max_age_hours INT NOT NULL DEFAULT 0,
    name_template VARCHAR(255) NOT NULL DEFAULT 'Auto {routine} {date}',
    ignored_files TEXT[] NOT NULL DEFAULT '{}',
    next_run TIMESTAMP,
    last_run TIMESTAMP,
    last_error TEXT,
    created TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

CREATE INDEX IF NOT EXISTS backup_plus_routines_server_idx
    ON backup_plus_routines (server_uuid, position);

ALTER TABLE backup_plus_backups
    ADD COLUMN IF NOT EXISTS routine_uuid UUID REFERENCES backup_plus_routines(uuid) ON DELETE SET NULL;

-- Bestehende Richtlinien (v1) in Routinen übersetzen – nur solange die v1-Spalten existieren,
-- damit die Migration auch bei einem erneuten Durchlauf (z. B. nach dem Wechsel der Paket-ID) funktioniert.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = current_schema() AND table_name = 'backup_plus_policies' AND column_name = 'interval_hours'
    ) THEN
        INSERT INTO backup_plus_routines (server_uuid, name, enabled, position, cron, source, keep_count, name_template, ignored_files)
        SELECT
            server_uuid,
            'Stündlich',
            TRUE,
            0,
            CASE WHEN interval_hours <= 1
                THEN format('0 %s * * * *', minute_offset)
                ELSE format('0 %s */%s * * *', minute_offset, interval_hours)
            END,
            'create',
            GREATEST(keep_hourly, 1),
            replace(name_template, '{tier}', '{routine}'),
            ignored_files
        FROM backup_plus_policies;

        INSERT INTO backup_plus_routines (server_uuid, name, enabled, position, cron, source, source_routine_uuid, keep_count)
        SELECT p.server_uuid, 'Täglich', TRUE, 1, format('0 %s 0 * * *', p.minute_offset), 'promote', r.uuid, p.keep_daily
        FROM backup_plus_policies p
        JOIN backup_plus_routines r ON r.server_uuid = p.server_uuid AND r.name = 'Stündlich'
        WHERE p.keep_daily > 0;

        INSERT INTO backup_plus_routines (server_uuid, name, enabled, position, cron, source, source_routine_uuid, keep_count)
        SELECT p.server_uuid, 'Wöchentlich', TRUE, 2, format('0 %s 0 * * 1', p.minute_offset), 'promote', r.uuid, p.keep_weekly
        FROM backup_plus_policies p
        JOIN backup_plus_routines r ON r.server_uuid = p.server_uuid AND r.name = 'Stündlich'
        WHERE p.keep_weekly > 0;

        INSERT INTO backup_plus_routines (server_uuid, name, enabled, position, cron, source, source_routine_uuid, keep_count)
        SELECT p.server_uuid, 'Monatlich', TRUE, 3, format('0 %s 0 1 * *', p.minute_offset), 'promote', r.uuid, p.keep_monthly
        FROM backup_plus_policies p
        JOIN backup_plus_routines r ON r.server_uuid = p.server_uuid AND r.name = 'Stündlich'
        WHERE p.keep_monthly > 0;
    END IF;

    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = current_schema() AND table_name = 'backup_plus_backups' AND column_name = 'tier'
    ) THEN
        UPDATE backup_plus_backups b
        SET routine_uuid = r.uuid
        FROM backup_plus_routines r
        WHERE r.server_uuid = b.server_uuid
            AND r.name = CASE b.tier
                WHEN 'daily' THEN 'Täglich'
                WHEN 'weekly' THEN 'Wöchentlich'
                WHEN 'monthly' THEN 'Monatlich'
                ELSE 'Stündlich'
            END;
    END IF;
END $$;

ALTER TABLE backup_plus_backups DROP COLUMN IF EXISTS tier;

ALTER TABLE backup_plus_policies
    DROP COLUMN IF EXISTS interval_hours,
    DROP COLUMN IF EXISTS minute_offset,
    DROP COLUMN IF EXISTS keep_hourly,
    DROP COLUMN IF EXISTS keep_daily,
    DROP COLUMN IF EXISTS keep_weekly,
    DROP COLUMN IF EXISTS keep_monthly,
    DROP COLUMN IF EXISTS name_template,
    DROP COLUMN IF EXISTS ignored_files,
    DROP COLUMN IF EXISTS next_run;

DROP INDEX IF EXISTS backup_plus_policies_enabled_next_run_idx;
