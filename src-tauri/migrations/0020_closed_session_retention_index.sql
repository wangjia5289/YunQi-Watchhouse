CREATE INDEX idx_sessions_closed_ended_at
  ON activity_sessions(ended_at_ms)
  WHERE is_open = 0;
