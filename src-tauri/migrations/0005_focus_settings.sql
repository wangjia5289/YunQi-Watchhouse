ALTER TABLE settings
  ADD COLUMN daily_focus_goal_minutes INTEGER NOT NULL DEFAULT 240
  CHECK (daily_focus_goal_minutes BETWEEN 0 AND 1440);

ALTER TABLE settings
  ADD COLUMN focus_block_gap_minutes INTEGER NOT NULL DEFAULT 5
  CHECK (focus_block_gap_minutes BETWEEN 1 AND 60);

ALTER TABLE settings
  ADD COLUMN break_reminders_enabled INTEGER NOT NULL DEFAULT 0
  CHECK (break_reminders_enabled IN (0, 1));

ALTER TABLE settings
  ADD COLUMN break_reminder_minutes INTEGER NOT NULL DEFAULT 60
  CHECK (break_reminder_minutes IN (30, 45, 60, 90, 120));

ALTER TABLE settings
  ADD COLUMN quiet_hours_start TEXT NOT NULL DEFAULT '22:00'
  CHECK (length(quiet_hours_start) = 5);

ALTER TABLE settings
  ADD COLUMN quiet_hours_end TEXT NOT NULL DEFAULT '08:00'
  CHECK (length(quiet_hours_end) = 5);
