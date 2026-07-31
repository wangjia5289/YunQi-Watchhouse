ALTER TABLE activity_sessions
  ADD COLUMN category_override TEXT
  CHECK (category_override IS NULL OR length(trim(category_override)) BETWEEN 1 AND 40);

CREATE TABLE category_rules (
  id            INTEGER PRIMARY KEY,
  match_field   TEXT NOT NULL
    CHECK (match_field IN ('APPLICATION_NAME', 'BUNDLE_ID', 'WINDOW_TITLE')),
  pattern       TEXT NOT NULL CHECK (length(trim(pattern)) BETWEEN 1 AND 120),
  category      TEXT NOT NULL CHECK (length(trim(category)) BETWEEN 1 AND 40),
  priority      INTEGER NOT NULL DEFAULT 100 CHECK (priority BETWEEN 0 AND 9999),
  enabled       INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_category_rules_priority
  ON category_rules(enabled, priority, id);

CREATE INDEX idx_sessions_category_override
  ON activity_sessions(category_override COLLATE NOCASE)
  WHERE category_override IS NOT NULL;
