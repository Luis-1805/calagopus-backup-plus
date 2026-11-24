-- Optionaler, vom Admin gesetzter Mindestabstand (Sekunden) nur für diesen Server.
-- NULL = globale Einstellung verwenden.
ALTER TABLE backup_plus_policies
    ADD COLUMN IF NOT EXISTS min_interval_seconds INT;
