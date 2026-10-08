-- Fail closed on legacy inconsistencies; never silently delete or reassign content.
CREATE TEMP TABLE vcms_api_ownership_guard (valid INTEGER CONSTRAINT api_site_ownership_valid CHECK(valid = 1));
INSERT INTO vcms_api_ownership_guard SELECT CASE WHEN EXISTS (
 SELECT 1 FROM entries e LEFT JOIN collections c ON c.id = e.collection_id
 WHERE c.id IS NULL OR c.site_id <> e.site_id OR
  (e.singleton_collection_id IS NOT NULL AND (e.singleton_collection_id <> e.collection_id OR c.is_singleton <> 1))
) OR EXISTS (
 SELECT 1 FROM entry_file_references r LEFT JOIN entries e ON e.id = r.entry_id LEFT JOIN files f ON f.id = r.file_id
 WHERE e.id IS NULL OR f.id IS NULL OR e.site_id <> r.site_id OR f.site_id <> r.site_id
) THEN 0 ELSE 1 END;
DROP TABLE vcms_api_ownership_guard;

-- Cross-site integrity for dynamic content and media references.
-- Service checks remain useful for friendly errors; these triggers are the
-- final boundary when a new adapter or offline job writes directly.

ALTER TABLE entry_file_references RENAME TO old_entry_file_references;
CREATE TABLE entry_file_references (
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    field_name TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (entry_id, file_id, field_name)
);
INSERT INTO entry_file_references (entry_id, file_id, site_id)
SELECT entry_id, file_id, site_id FROM old_entry_file_references;
DROP TABLE old_entry_file_references;
CREATE INDEX idx_efr_file ON entry_file_references(file_id);
CREATE INDEX idx_efr_entry ON entry_file_references(entry_id);

CREATE TRIGGER IF NOT EXISTS entries_collection_same_site_insert
BEFORE INSERT ON entries
WHEN EXISTS (
    SELECT 1 FROM collections c
    WHERE c.id = NEW.collection_id AND c.site_id <> NEW.site_id
)
BEGIN
    SELECT RAISE(ABORT, 'entry collection belongs to another site');
END;

CREATE TRIGGER IF NOT EXISTS entries_collection_same_site_update
BEFORE UPDATE OF site_id, collection_id ON entries
WHEN EXISTS (
    SELECT 1 FROM collections c
    WHERE c.id = NEW.collection_id AND c.site_id <> NEW.site_id
)
BEGIN
    SELECT RAISE(ABORT, 'entry collection belongs to another site');
END;

CREATE TRIGGER IF NOT EXISTS entry_file_reference_same_site_insert
BEFORE INSERT ON entry_file_references
WHEN EXISTS (
    SELECT 1 FROM entries e JOIN files f ON f.id = NEW.file_id
    WHERE e.id = NEW.entry_id AND (e.site_id <> NEW.site_id OR f.site_id <> NEW.site_id)
)
BEGIN
    SELECT RAISE(ABORT, 'entry and file reference belong to another site');
END;

CREATE TRIGGER IF NOT EXISTS entry_file_reference_same_site_update
BEFORE UPDATE OF entry_id, file_id, site_id ON entry_file_references
WHEN EXISTS (
    SELECT 1 FROM entries e JOIN files f ON f.id = NEW.file_id
    WHERE e.id = NEW.entry_id AND (e.site_id <> NEW.site_id OR f.site_id <> NEW.site_id)
)
BEGIN
    SELECT RAISE(ABORT, 'entry and file reference belong to another site');
END;

ALTER TABLE entries ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
CREATE TRIGGER entries_version AFTER UPDATE ON entries
WHEN NEW.version = OLD.version
BEGIN
 UPDATE entries SET version = OLD.version + 1 WHERE id = NEW.id;
END;

CREATE TRIGGER entries_singleton_insert BEFORE INSERT ON entries
WHEN NEW.singleton_collection_id IS NOT NULL AND (NEW.singleton_collection_id <> NEW.collection_id OR NOT EXISTS (
 SELECT 1 FROM collections WHERE id = NEW.singleton_collection_id AND site_id = NEW.site_id AND is_singleton = 1
))
BEGIN SELECT RAISE(ABORT, 'invalid singleton collection'); END;
CREATE TRIGGER entries_singleton_update BEFORE UPDATE OF collection_id, singleton_collection_id, site_id ON entries
WHEN NEW.singleton_collection_id IS NOT NULL AND (NEW.singleton_collection_id <> NEW.collection_id OR NOT EXISTS (
 SELECT 1 FROM collections WHERE id = NEW.singleton_collection_id AND site_id = NEW.site_id AND is_singleton = 1
))
BEGIN SELECT RAISE(ABORT, 'invalid singleton collection'); END;
CREATE TRIGGER collections_site_update BEFORE UPDATE OF site_id ON collections
WHEN EXISTS (SELECT 1 FROM entries WHERE collection_id = OLD.id AND site_id <> NEW.site_id)
BEGIN SELECT RAISE(ABORT, 'collection has entries in another site'); END;
CREATE TRIGGER entries_site_update BEFORE UPDATE OF site_id ON entries
WHEN EXISTS (SELECT 1 FROM entry_file_references WHERE entry_id = OLD.id AND site_id <> NEW.site_id)
BEGIN SELECT RAISE(ABORT, 'entry has references in another site'); END;
CREATE TRIGGER files_site_update BEFORE UPDATE OF site_id ON files
WHEN EXISTS (SELECT 1 FROM entry_file_references WHERE file_id = OLD.id AND site_id <> NEW.site_id)
BEGIN SELECT RAISE(ABORT, 'file has references in another site'); END;

-- Signed upload use survives file deletion and server restarts. These security
-- records are deliberately excluded from logical content backups.
CREATE TABLE signed_upload_uses (
    file_id TEXT PRIMARY KEY,
    expires_at BIGINT NOT NULL
);
CREATE INDEX idx_signed_upload_uses_expiry ON signed_upload_uses(expires_at);
