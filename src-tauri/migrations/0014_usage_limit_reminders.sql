CREATE TABLE usage_limit_daily_exceptions (
  rule_id                            INTEGER NOT NULL
    REFERENCES usage_limit_rules(id) ON DELETE CASCADE,
  local_date                         TEXT NOT NULL
    CHECK (local_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
  temporary_added_minutes            INTEGER NOT NULL DEFAULT 0
    CHECK (temporary_added_minutes BETWEEN 0 AND 1440),
  notifications_snoozed_until_ms     INTEGER,
  notifications_silenced             INTEGER NOT NULL DEFAULT 0
    CHECK (notifications_silenced IN (0, 1)),
  created_at_ms                      INTEGER NOT NULL,
  updated_at_ms                      INTEGER NOT NULL,
  PRIMARY KEY (rule_id, local_date)
);

CREATE INDEX idx_usage_limit_daily_exceptions_date
  ON usage_limit_daily_exceptions(local_date);

CREATE TABLE usage_limit_reminder_history (
  id                                 INTEGER PRIMARY KEY AUTOINCREMENT,
  rule_id                            INTEGER NOT NULL,
  scope_type                         TEXT NOT NULL
    CHECK (scope_type IN ('APPLICATION', 'CATEGORY')),
  application_id                     INTEGER,
  application_name                   TEXT,
  category                           TEXT,
  target_name                        TEXT NOT NULL,
  local_date                         TEXT NOT NULL
    CHECK (local_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
  threshold                          INTEGER NOT NULL CHECK (threshold IN (80, 100)),
  delivered_at_ms                    INTEGER NOT NULL,
  CHECK (
    (scope_type = 'APPLICATION' AND application_id IS NOT NULL
      AND application_name IS NOT NULL AND category IS NULL)
    OR
    (scope_type = 'CATEGORY' AND application_id IS NULL
      AND application_name IS NULL AND category IS NOT NULL)
  )
);

CREATE INDEX idx_usage_limit_reminder_history_date
  ON usage_limit_reminder_history(local_date, delivered_at_ms DESC);

INSERT INTO usage_limit_reminder_history (
  rule_id, scope_type, application_id, application_name, category,
  target_name, local_date, threshold, delivered_at_ms
)
SELECT
  alerts.rule_id,
  rules.scope_type,
  rules.application_id,
  applications.name,
  rules.category,
  COALESCE(applications.name, rules.category),
  alerts.local_date,
  alerts.threshold,
  alerts.delivered_at_ms
FROM usage_limit_alerts AS alerts
JOIN usage_limit_rules AS rules ON rules.id = alerts.rule_id
LEFT JOIN applications ON applications.id = rules.application_id
WHERE NOT (
  alerts.threshold = 80
  AND EXISTS (
    SELECT 1
    FROM usage_limit_alerts AS reached
    WHERE reached.rule_id = alerts.rule_id
      AND reached.local_date = alerts.local_date
      AND reached.delivered_at_ms = alerts.delivered_at_ms
      AND reached.threshold = 100
  )
);
