ALTER TABLE activity_sessions ADD COLUMN note TEXT;

CREATE TABLE timeline_undo (
  token         TEXT PRIMARY KEY,
  snapshot_json TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_timeline_undo_created_at
  ON timeline_undo(created_at_ms);
