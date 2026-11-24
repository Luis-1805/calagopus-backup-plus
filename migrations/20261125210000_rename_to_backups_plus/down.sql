-- Nur die Tabellen zurückbenennen, damit die älteren down-Migrationen sie finden.
DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY['policies', 'backups', 'log', 'routines'] LOOP
        IF to_regclass('backups_plus_' || t) IS NOT NULL AND to_regclass('backup_plus_' || t) IS NULL THEN
            EXECUTE format('ALTER TABLE %I RENAME TO %I', 'backups_plus_' || t, 'backup_plus_' || t);
        END IF;
    END LOOP;

    FOREACH t IN ARRAY ARRAY['backups_server_idx', 'log_server_created_idx', 'routines_server_idx'] LOOP
        IF to_regclass('backups_plus_' || t) IS NOT NULL AND to_regclass('backup_plus_' || t) IS NULL THEN
            EXECUTE format('ALTER INDEX %I RENAME TO %I', 'backups_plus_' || t, 'backup_plus_' || t);
        END IF;
    END LOOP;
END $$;
