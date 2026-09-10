CREATE TABLE history (
    entry_id TEXT NOT NULL,
    "index" INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    opened_at TEXT NOT NULL,
    saved_at TEXT,
    status_after TEXT NOT NULL,
    PRIMARY KEY (entry_id, "index")
);
