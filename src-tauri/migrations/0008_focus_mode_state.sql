ALTER TABLE settings
  ADD COLUMN focus_mode_active INTEGER NOT NULL DEFAULT 0
  CHECK (focus_mode_active IN (0, 1));

ALTER TABLE settings
  ADD COLUMN focus_mode_started_at_ms INTEGER;
