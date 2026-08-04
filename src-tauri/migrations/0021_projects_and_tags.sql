CREATE TABLE projects (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL COLLATE NOCASE
    CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 80),
  color         TEXT NOT NULL
    CHECK (
      length(color) = 7
      AND substr(color, 1, 1) = '#'
      AND substr(color, 2) NOT GLOB '*[^0-9A-F]*'
    ),
  archived      INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms)
);

CREATE UNIQUE INDEX idx_projects_name_nocase
  ON projects(name COLLATE NOCASE);
CREATE INDEX idx_projects_archived_name
  ON projects(archived, name COLLATE NOCASE, id);

CREATE TABLE activity_tags (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL COLLATE NOCASE
    CHECK (name = trim(name) AND length(name) BETWEEN 1 AND 80),
  color         TEXT NOT NULL
    CHECK (
      length(color) = 7
      AND substr(color, 1, 1) = '#'
      AND substr(color, 2) NOT GLOB '*[^0-9A-F]*'
    ),
  archived      INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms)
);

CREATE UNIQUE INDEX idx_activity_tags_name_nocase
  ON activity_tags(name COLLATE NOCASE);
CREATE INDEX idx_activity_tags_archived_name
  ON activity_tags(archived, name COLLATE NOCASE, id);

CREATE TABLE session_projects (
  session_id INTEGER PRIMARY KEY
    REFERENCES activity_sessions(id) ON DELETE CASCADE,
  project_id INTEGER NOT NULL
    REFERENCES projects(id) ON DELETE RESTRICT
);

CREATE INDEX idx_session_projects_project
  ON session_projects(project_id, session_id);

CREATE TABLE session_tags (
  session_id INTEGER NOT NULL
    REFERENCES activity_sessions(id) ON DELETE CASCADE,
  tag_id     INTEGER NOT NULL
    REFERENCES activity_tags(id) ON DELETE RESTRICT,
  PRIMARY KEY (session_id, tag_id)
);

CREATE INDEX idx_session_tags_tag
  ON session_tags(tag_id, session_id);
