CREATE TABLE category_rule_reapply_undo (
  token         TEXT PRIMARY KEY,
  snapshot_json TEXT NOT NULL CHECK (length(snapshot_json) > 0),
  created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_category_rule_reapply_undo_created
  ON category_rule_reapply_undo(created_at_ms);
