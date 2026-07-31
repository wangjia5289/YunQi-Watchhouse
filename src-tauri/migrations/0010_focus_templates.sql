CREATE TABLE focus_plan_templates (
  id               INTEGER PRIMARY KEY,
  name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 40),
  duration_minutes INTEGER NOT NULL CHECK (duration_minutes BETWEEN 5 AND 240),
  created_at_ms    INTEGER NOT NULL,
  updated_at_ms    INTEGER NOT NULL
);

INSERT INTO focus_plan_templates (name, duration_minutes, created_at_ms, updated_at_ms)
VALUES
  ('Pomodoro', 25, 0, 0),
  ('Deep work', 50, 0, 0);
