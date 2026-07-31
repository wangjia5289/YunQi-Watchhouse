CREATE TABLE usage_limit_rules (
  id                       INTEGER PRIMARY KEY AUTOINCREMENT,
  scope_type               TEXT NOT NULL
    CHECK (scope_type IN ('APPLICATION', 'CATEGORY')),
  application_id           INTEGER REFERENCES applications(id) ON DELETE CASCADE,
  category                 TEXT,
  weekday_limit_minutes    INTEGER NOT NULL
    CHECK (weekday_limit_minutes BETWEEN 1 AND 1440),
  weekend_limit_minutes    INTEGER NOT NULL
    CHECK (weekend_limit_minutes BETWEEN 1 AND 1440),
  notifications_enabled    INTEGER NOT NULL DEFAULT 1
    CHECK (notifications_enabled IN (0, 1)),
  enabled                  INTEGER NOT NULL DEFAULT 1
    CHECK (enabled IN (0, 1)),
  created_at_ms            INTEGER NOT NULL,
  updated_at_ms            INTEGER NOT NULL,
  CHECK (
    (scope_type = 'APPLICATION' AND application_id IS NOT NULL AND category IS NULL)
    OR
    (scope_type = 'CATEGORY' AND application_id IS NULL
      AND category IS NOT NULL AND length(trim(category)) BETWEEN 1 AND 40)
  )
);

CREATE UNIQUE INDEX idx_usage_limit_rules_application
  ON usage_limit_rules(application_id)
  WHERE scope_type = 'APPLICATION';

CREATE UNIQUE INDEX idx_usage_limit_rules_category
  ON usage_limit_rules(category COLLATE NOCASE)
  WHERE scope_type = 'CATEGORY';

CREATE TABLE usage_limit_alerts (
  rule_id          INTEGER NOT NULL
    REFERENCES usage_limit_rules(id) ON DELETE CASCADE,
  local_date       TEXT NOT NULL CHECK (length(local_date) = 10),
  threshold        INTEGER NOT NULL CHECK (threshold IN (80, 100)),
  delivered_at_ms INTEGER NOT NULL,
  PRIMARY KEY (rule_id, local_date, threshold)
);

CREATE INDEX idx_usage_limit_alerts_date
  ON usage_limit_alerts(local_date);
