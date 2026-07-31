CREATE TABLE shortcut_settings (
  singleton_id       INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  toggle_focus       TEXT,
  pause_focus        TEXT,
  start_template     TEXT,
  updated_at_ms      INTEGER NOT NULL
);

INSERT INTO shortcut_settings (
  singleton_id, toggle_focus, pause_focus, start_template, updated_at_ms
) VALUES (
  1, 'CommandOrControl+Shift+F', 'CommandOrControl+Shift+P',
  'CommandOrControl+Shift+1', 0
);
