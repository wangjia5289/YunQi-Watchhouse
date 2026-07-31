ALTER TABLE settings
  ADD COLUMN retention_days INTEGER NOT NULL DEFAULT 0
  CHECK (retention_days IN (0, 30, 90, 180, 365));

ALTER TABLE settings
  ADD COLUMN automatic_backup_enabled INTEGER NOT NULL DEFAULT 0
  CHECK (automatic_backup_enabled IN (0, 1));

ALTER TABLE settings
  ADD COLUMN backup_interval TEXT NOT NULL DEFAULT 'WEEKLY'
  CHECK (backup_interval IN ('DAILY', 'WEEKLY'));

ALTER TABLE settings
  ADD COLUMN backup_keep_count INTEGER NOT NULL DEFAULT 5
  CHECK (backup_keep_count BETWEEN 1 AND 20);

ALTER TABLE settings
  ADD COLUMN backup_directory TEXT;

ALTER TABLE settings
  ADD COLUMN last_maintenance_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE settings
  ADD COLUMN last_backup_at_ms INTEGER NOT NULL DEFAULT 0;
