CREATE TABLE ocr_results (
    page_id TEXT NOT NULL,
    model_version TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (page_id, model_version)
);
