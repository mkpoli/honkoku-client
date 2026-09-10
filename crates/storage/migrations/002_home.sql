CREATE TABLE users (
  id TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  updated_at TEXT,
  fetched_at TEXT NOT NULL
);
CREATE TABLE timeline_events (
  id TEXT PRIMARY KEY,
  parent_id TEXT NOT NULL,
  payload TEXT NOT NULL,
  updated_at TEXT,
  fetched_at TEXT NOT NULL
);
CREATE INDEX timeline_events_project ON timeline_events(parent_id);
