-- Composite indexes for hot list/filter paths.
-- entries are frequently filtered by (site_id, status); files by (site_id, deleted_at).
CREATE INDEX idx_entries_site_status ON entries(site_id, status);
CREATE INDEX idx_files_site_deleted ON files(site_id, deleted_at);
