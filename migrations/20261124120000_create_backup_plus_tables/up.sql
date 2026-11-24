CREATE TABLE IF NOT EXISTS backup_plus_policies (
    server_uuid UUID PRIMARY KEY REFERENCES servers(uuid) ON DELETE CASCADE,
    backup_group_uuid UUID REFERENCES server_backup_groups(uuid) ON DELETE SET NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    interval_hours INT NOT NULL DEFAULT 1,
    minute_offset INT NOT NULL DEFAULT 0,
    keep_hourly INT NOT NULL DEFAULT 3,
    keep_daily INT NOT NULL DEFAULT 0,
    keep_weekly INT NOT NULL DEFAULT 1,
    keep_monthly INT NOT NULL DEFAULT 1,
    name_template VARCHAR(255) NOT NULL DEFAULT 'Auto {date}',
    ignored_files TEXT[] NOT NULL DEFAULT '{}',
    delete_failed BOOLEAN NOT NULL DEFAULT TRUE,
    next_run TIMESTAMP,
    last_run TIMESTAMP,
    last_error TEXT,
    created TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
    updated TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

CREATE INDEX IF NOT EXISTS backup_plus_policies_enabled_next_run_idx
    ON backup_plus_policies (enabled, next_run);

CREATE TABLE IF NOT EXISTS backup_plus_backups (
    backup_uuid UUID PRIMARY KEY REFERENCES server_backups(uuid) ON DELETE CASCADE,
    server_uuid UUID NOT NULL REFERENCES servers(uuid) ON DELETE CASCADE,
    tier VARCHAR(16) NOT NULL DEFAULT 'hourly',
    promoted TIMESTAMP,
    created TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

CREATE INDEX IF NOT EXISTS backup_plus_backups_server_idx
    ON backup_plus_backups (server_uuid);

CREATE TABLE IF NOT EXISTS backup_plus_log (
    id BIGSERIAL PRIMARY KEY,
    server_uuid UUID NOT NULL REFERENCES servers(uuid) ON DELETE CASCADE,
    level VARCHAR(16) NOT NULL,
    message TEXT NOT NULL,
    created TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

CREATE INDEX IF NOT EXISTS backup_plus_log_server_created_idx
    ON backup_plus_log (server_uuid, created DESC);
