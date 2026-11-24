-- Optional: übernommene Backups beim Wechsel in eine Routine umbenennen.
ALTER TABLE backup_plus_routines
    ADD COLUMN IF NOT EXISTS rename_template VARCHAR(255);
