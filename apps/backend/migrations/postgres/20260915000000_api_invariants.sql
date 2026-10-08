-- Fail closed on legacy inconsistencies; never silently delete or reassign content.
DO $$
BEGIN
 IF EXISTS (
 SELECT 1 FROM entries e LEFT JOIN collections c ON c.id = e.collection_id
 WHERE c.id IS NULL OR c.site_id <> e.site_id OR
  (e.singleton_collection_id IS NOT NULL AND (e.singleton_collection_id <> e.collection_id OR NOT c.is_singleton))
) OR EXISTS (
 SELECT 1 FROM entry_file_references r LEFT JOIN entries e ON e.id = r.entry_id LEFT JOIN files f ON f.id = r.file_id
 WHERE e.id IS NULL OR f.id IS NULL OR e.site_id <> r.site_id OR f.site_id <> r.site_id
) THEN RAISE EXCEPTION 'Existing content violates API site ownership; repair it before upgrading'; END IF;
END;
$$;

-- Cross-site integrity for dynamic content and media references.

ALTER TABLE entry_file_references ADD COLUMN IF NOT EXISTS field_name TEXT NOT NULL DEFAULT '';
ALTER TABLE entry_file_references DROP CONSTRAINT entry_file_references_pkey;
ALTER TABLE entry_file_references ADD PRIMARY KEY (entry_id, file_id, field_name);

CREATE OR REPLACE FUNCTION vcms_entries_collection_same_site() RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM collections c
        WHERE c.id = NEW.collection_id AND c.site_id <> NEW.site_id
    ) THEN
        RAISE EXCEPTION 'entry collection belongs to another site';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS entries_collection_same_site ON entries;
CREATE TRIGGER entries_collection_same_site
BEFORE INSERT OR UPDATE OF site_id, collection_id ON entries
FOR EACH ROW EXECUTE FUNCTION vcms_entries_collection_same_site();

CREATE OR REPLACE FUNCTION vcms_entry_file_reference_same_site() RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM entries e JOIN files f ON f.id = NEW.file_id
        WHERE e.id = NEW.entry_id AND (e.site_id <> NEW.site_id OR f.site_id <> NEW.site_id)
    ) THEN
        RAISE EXCEPTION 'entry and file reference belong to another site';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS entry_file_reference_same_site ON entry_file_references;
CREATE TRIGGER entry_file_reference_same_site
BEFORE INSERT OR UPDATE ON entry_file_references
FOR EACH ROW EXECUTE FUNCTION vcms_entry_file_reference_same_site();

ALTER TABLE entries ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
CREATE FUNCTION vcms_entry_version() RETURNS trigger AS $$
BEGIN
 NEW.version := OLD.version + 1;
 RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER entries_version BEFORE UPDATE ON entries
FOR EACH ROW EXECUTE FUNCTION vcms_entry_version();

CREATE FUNCTION vcms_entry_singleton() RETURNS trigger AS $$
BEGIN
 IF NEW.singleton_collection_id IS NOT NULL AND (NEW.singleton_collection_id <> NEW.collection_id OR NOT EXISTS (
  SELECT 1 FROM collections WHERE id = NEW.singleton_collection_id AND site_id = NEW.site_id AND is_singleton
 )) THEN RAISE EXCEPTION 'invalid singleton collection'; END IF;
 RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER entries_singleton BEFORE INSERT OR UPDATE OF collection_id, singleton_collection_id, site_id ON entries
FOR EACH ROW EXECUTE FUNCTION vcms_entry_singleton();
CREATE FUNCTION vcms_parent_site() RETURNS trigger AS $$
BEGIN
 IF TG_TABLE_NAME = 'collections' THEN
  IF EXISTS (SELECT 1 FROM entries WHERE collection_id = OLD.id AND site_id <> NEW.site_id) THEN
   RAISE EXCEPTION 'collection has entries in another site';
  END IF;
 ELSIF TG_TABLE_NAME = 'entries' THEN
  IF EXISTS (SELECT 1 FROM entry_file_references WHERE entry_id = OLD.id AND site_id <> NEW.site_id) THEN
   RAISE EXCEPTION 'entry has references in another site';
  END IF;
 ELSE
  IF EXISTS (SELECT 1 FROM entry_file_references WHERE file_id = OLD.id AND site_id <> NEW.site_id) THEN
   RAISE EXCEPTION 'file has references in another site';
  END IF;
 END IF;
 RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER collections_site_update BEFORE UPDATE OF site_id ON collections FOR EACH ROW EXECUTE FUNCTION vcms_parent_site();
CREATE TRIGGER entries_site_update BEFORE UPDATE OF site_id ON entries FOR EACH ROW EXECUTE FUNCTION vcms_parent_site();
CREATE TRIGGER files_site_update BEFORE UPDATE OF site_id ON files FOR EACH ROW EXECUTE FUNCTION vcms_parent_site();

-- Signed upload use survives file deletion and server restarts. These security
-- records are deliberately excluded from logical content backups.
CREATE TABLE signed_upload_uses (
    file_id TEXT PRIMARY KEY,
    expires_at BIGINT NOT NULL
);
CREATE INDEX idx_signed_upload_uses_expiry ON signed_upload_uses(expires_at);
