CREATE TABLE search_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE search_queue (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id TEXT NOT NULL UNIQUE,
    deleted INTEGER NOT NULL DEFAULT 0
);
INSERT INTO search_queue(page_id) SELECT id FROM pages;
CREATE TRIGGER search_page_insert AFTER INSERT ON pages BEGIN
    DELETE FROM search_queue WHERE page_id=NEW.id;
    INSERT INTO search_queue(page_id) VALUES(NEW.id);
END;
CREATE TRIGGER search_page_update AFTER UPDATE ON pages BEGIN
    DELETE FROM search_queue WHERE page_id=NEW.id;
    INSERT INTO search_queue(page_id) VALUES(NEW.id);
END;
CREATE TRIGGER search_page_delete AFTER DELETE ON pages BEGIN
    DELETE FROM search_queue WHERE page_id=OLD.id;
    INSERT INTO search_queue(page_id,deleted) VALUES(OLD.id,1);
END;
