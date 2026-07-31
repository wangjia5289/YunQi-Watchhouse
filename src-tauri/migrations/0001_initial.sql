CREATE TABLE applications (
  id                INTEGER PRIMARY KEY,
  identity_key      TEXT NOT NULL UNIQUE,
  name              TEXT NOT NULL CHECK (length(name) > 0),
  bundle_id         TEXT,
  executable_path   TEXT,
  first_seen_at_ms  INTEGER NOT NULL,
  last_seen_at_ms   INTEGER NOT NULL,
  CHECK (last_seen_at_ms >= first_seen_at_ms)
);

CREATE TABLE activity_sessions (
  id                INTEGER PRIMARY KEY,
  state             TEXT NOT NULL CHECK (state IN ('ACTIVE', 'IDLE')),
  application_id    INTEGER REFERENCES applications(id),
  window_title      TEXT,
  started_at_ms     INTEGER NOT NULL,
  ended_at_ms       INTEGER NOT NULL,
  duration_ms       INTEGER NOT NULL CHECK (duration_ms >= 0),
  is_open           INTEGER NOT NULL DEFAULT 1 CHECK (is_open IN (0, 1)),
  closed_reason     TEXT,
  created_at_ms     INTEGER NOT NULL,
  updated_at_ms     INTEGER NOT NULL,
  CHECK (
    (state = 'ACTIVE' AND application_id IS NOT NULL) OR
    (state = 'IDLE' AND application_id IS NULL)
  ),
  CHECK (ended_at_ms >= started_at_ms),
  CHECK (duration_ms = ended_at_ms - started_at_ms),
  CHECK (
    (is_open = 1 AND closed_reason IS NULL) OR
    (is_open = 0 AND closed_reason IS NOT NULL)
  )
);

CREATE INDEX idx_sessions_overlap
  ON activity_sessions(started_at_ms, ended_at_ms);
CREATE INDEX idx_sessions_application_range
  ON activity_sessions(application_id, started_at_ms, ended_at_ms);
CREATE UNIQUE INDEX idx_sessions_one_open
  ON activity_sessions(is_open) WHERE is_open = 1;

CREATE TABLE settings (
  singleton_id                 INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  idle_threshold_seconds       INTEGER NOT NULL DEFAULT 180
    CHECK (idle_threshold_seconds BETWEEN 30 AND 3600),
  launch_at_login              INTEGER NOT NULL DEFAULT 0
    CHECK (launch_at_login IN (0, 1)),
  start_tracking_automatically INTEGER NOT NULL DEFAULT 1
    CHECK (start_tracking_automatically IN (0, 1)),
  hide_to_tray_on_close        INTEGER NOT NULL DEFAULT 1
    CHECK (hide_to_tray_on_close IN (0, 1)),
  record_window_titles         INTEGER NOT NULL DEFAULT 0
    CHECK (record_window_titles IN (0, 1)),
  appearance                   TEXT NOT NULL DEFAULT 'SYSTEM'
    CHECK (appearance IN ('SYSTEM', 'LIGHT', 'DARK')),
  updated_at_ms                INTEGER NOT NULL
);

INSERT INTO settings (singleton_id, updated_at_ms) VALUES (1, 0);
