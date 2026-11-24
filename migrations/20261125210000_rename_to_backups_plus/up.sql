-- Umzug von "Backup Plus" (dev.backupplus.gfsbackups) zu "Backups Plus" (dev.backupsplus.backups).
-- Bei einer Neuinstallation benennt diese Migration nur die eigenen Tabellen um; alles andere findet nichts.
-- Jeder Schritt ist wiederholbar.

-- 1) Eigene Tabellen und Indizes umbenennen.
DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY['policies', 'backups', 'log', 'routines'] LOOP
        IF to_regclass('backup_plus_' || t) IS NOT NULL AND to_regclass('backups_plus_' || t) IS NULL THEN
            EXECUTE format('ALTER TABLE %I RENAME TO %I', 'backup_plus_' || t, 'backups_plus_' || t);
        END IF;
    END LOOP;

    FOREACH t IN ARRAY ARRAY['backups_server_idx', 'log_server_created_idx', 'routines_server_idx'] LOOP
        IF to_regclass('backup_plus_' || t) IS NOT NULL AND to_regclass('backups_plus_' || t) IS NULL THEN
            EXECUTE format('ALTER INDEX %I RENAME TO %I', 'backup_plus_' || t, 'backups_plus_' || t);
        END IF;
    END LOOP;
END $$;

-- 2) Einstellungen der alten Paket-ID übernehmen (Schlüssel "<paket>::<feld>").
DELETE FROM settings n
WHERE n.key LIKE '%dev.backupsplus.backups::%'
    AND EXISTS (
        SELECT 1 FROM settings o
        WHERE o.key = replace(n.key, 'dev.backupsplus.backups::', 'dev.backupplus.gfsbackups::')
    );

UPDATE settings
SET key = replace(key, 'dev.backupplus.gfsbackups::', 'dev.backupsplus.backups::')
WHERE key LIKE '%dev.backupplus.gfsbackups::%';

-- Freigegebene Extension-Rechte und deaktivierte Extensions verweisen auf die Paket-ID.
UPDATE settings
SET value = replace(
    replace(value, '"dev.backupplus.gfsbackups"', '"dev.backupsplus.backups"'),
    '"backup-plus.', '"backups-plus.'
)
WHERE key = '::extension_permissions'
    AND (value LIKE '%dev.backupplus.gfsbackups%' OR value LIKE '%backup-plus.%');

UPDATE settings
SET value = replace(value, 'dev.backupplus.gfsbackups', 'dev.backupsplus.backups')
WHERE key = '::disabled_extensions'
    AND value LIKE '%dev.backupplus.gfsbackups%';

-- 3) Rechte "backup-plus.*" in allen Rechte-Listen (Subuser, Rollen, API-Keys, …) umbenennen.
DO $$
DECLARE
    c RECORD;
BEGIN
    FOR c IN
        SELECT table_name, column_name
        FROM information_schema.columns
        WHERE table_schema = current_schema()
            AND column_name LIKE '%permissions'
            AND data_type = 'ARRAY'
            AND table_name NOT LIKE 'backups_plus_%'
    LOOP
        EXECUTE format(
            'UPDATE %I SET %I = array_replace(array_replace(%I, %L, %L), %L, %L) WHERE %I::text[] && %L::text[]',
            c.table_name, c.column_name, c.column_name,
            'backup-plus.read', 'backups-plus.read',
            'backup-plus.manage', 'backups-plus.manage',
            c.column_name, '{backup-plus.read,backup-plus.manage}'
        );
    END LOOP;
END $$;

-- 4) Sichtbaren Namen der Backup-Gruppen anpassen.
UPDATE server_backup_groups
SET name = 'Backups Plus (auto)'
WHERE name = 'Backup Plus (auto)'
    AND uuid IN (SELECT backup_group_uuid FROM backups_plus_policies WHERE backup_group_uuid IS NOT NULL);

-- 5) Einträge der alten Paket-ID in der Migrationstabelle entfernen (IDs = Hash aus Paket-ID + Name).
DELETE FROM migrations
WHERE id IN (
    '672c9f4e-d505-8f22-9e28-07e16e2c0934',
    '7db26366-b22f-8ea5-bbcb-6601e9ddf97f',
    'aa706e1d-9247-8b22-9138-eebb5c3376d0'
);
