CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  updated_at TEXT,
  fetched_at TEXT NOT NULL
);
CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  parent_id TEXT,
  payload TEXT NOT NULL,
  updated_at TEXT,
  fetched_at TEXT NOT NULL
);
CREATE INDEX collections_parent ON collections(parent_id);
CREATE TABLE entries (
  id TEXT PRIMARY KEY,
  parent_id TEXT,
  payload TEXT NOT NULL,
  updated_at TEXT,
  fetched_at TEXT NOT NULL
);
CREATE INDEX entries_parent ON entries(parent_id);
CREATE TABLE pages (
  id TEXT PRIMARY KEY,
  parent_id TEXT,
  entry_id TEXT NOT NULL,
  idx INTEGER NOT NULL,
  status TEXT NOT NULL,
  payload TEXT NOT NULL,
  updated_at TEXT,
  fetched_at TEXT NOT NULL
);
CREATE INDEX pages_parent ON pages(parent_id);
CREATE INDEX pages_entry_status ON pages(entry_id, status);
-- Complete endpoint snapshots distinguish list summaries from detail documents,
-- and record successful empty results without inferring completeness from rows.
CREATE TABLE responses (
  key TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  fetched_at TEXT NOT NULL
);
