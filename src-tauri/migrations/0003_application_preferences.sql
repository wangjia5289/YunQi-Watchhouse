ALTER TABLE applications
  ADD COLUMN category TEXT NOT NULL DEFAULT 'Uncategorized'
  CHECK (length(category) BETWEEN 1 AND 40);

ALTER TABLE applications
  ADD COLUMN is_ignored INTEGER NOT NULL DEFAULT 0
  CHECK (is_ignored IN (0, 1));
