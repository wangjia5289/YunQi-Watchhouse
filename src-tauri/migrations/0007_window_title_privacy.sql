ALTER TABLE applications
  ADD COLUMN record_window_titles INTEGER NOT NULL DEFAULT 0
  CHECK (record_window_titles IN (0, 1));
