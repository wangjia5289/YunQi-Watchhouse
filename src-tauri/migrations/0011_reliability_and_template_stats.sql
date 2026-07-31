ALTER TABLE focus_plan_templates ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE focus_plan_templates ADD COLUMN use_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE focus_plan_templates ADD COLUMN completed_count INTEGER NOT NULL DEFAULT 0;
UPDATE focus_plan_templates SET sort_order = id;
ALTER TABLE settings ADD COLUMN focus_plan_template_id INTEGER REFERENCES focus_plan_templates(id);
ALTER TABLE focus_plan_history ADD COLUMN template_id INTEGER REFERENCES focus_plan_templates(id);

CREATE TABLE data_health_undo (
  singleton_id  INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  snapshot_json TEXT NOT NULL,
  backup_path   TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL
);
