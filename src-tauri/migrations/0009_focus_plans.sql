ALTER TABLE settings ADD COLUMN focus_plan_end_at_ms INTEGER;
ALTER TABLE settings ADD COLUMN focus_plan_paused INTEGER NOT NULL DEFAULT 0
  CHECK (focus_plan_paused IN (0, 1));
ALTER TABLE settings ADD COLUMN focus_plan_paused_at_ms INTEGER;
ALTER TABLE settings ADD COLUMN focus_plan_total_paused_ms INTEGER NOT NULL DEFAULT 0;

CREATE TABLE focus_plan_history (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at_ms       INTEGER NOT NULL,
  planned_end_at_ms   INTEGER,
  ended_at_ms         INTEGER NOT NULL,
  paused_duration_ms  INTEGER NOT NULL DEFAULT 0,
  outcome             TEXT NOT NULL CHECK (outcome IN ('COMPLETED', 'CANCELLED'))
);

CREATE INDEX idx_focus_plan_history_started_at
  ON focus_plan_history(started_at_ms);
