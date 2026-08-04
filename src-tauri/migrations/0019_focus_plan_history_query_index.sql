CREATE INDEX idx_focus_plan_history_ended_at_id
  ON focus_plan_history(ended_at_ms DESC, id DESC);
