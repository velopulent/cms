//! Full-text search over entries, backed by an embedded [Tantivy] index.
//!
//! The index is a **derived** view of the `entries` table — the database stays the
//! source of truth and the index can always be rebuilt from it
//! (see [`SearchService::rebuild_all`]). Queries are ranked with BM25 over
//! English-stemmed tokens (so "running" matches "run"). Typo tolerance is left as a
//! follow-up: Tantivy's fuzzy queries score by constant, which would flatten ranking.
//!
//! ## Single writer, many readers, cross-process sync
//!
//! Tantivy permits one `IndexWriter` per directory. Rather than let that limit who
//! can search, we split the roles:
//!
//! - **Reading** needs no lock. Any process opens the index [read-only]
//!   ([`SearchService::open_read_only`]) and gets full ranked search — including a
//!   separate `cms mcp stdio` process running alongside the server.
//! - **Writing** goes through a durable database queue ([`queue`]) instead of the
//!   index directly. Any process enqueues on a content change; the one running
//!   server owns the writer ([`SearchService::open`]) and is the sole consumer
//!   ([`indexer`]) that drains the queue into the index. This makes sync work across
//!   processes and survive restarts, while keeping the embedded single-writer model.
//!
//! [Tantivy]: https://github.com/quickwit-oss/tantivy

pub mod indexer;
pub mod queue;
mod schema;

use std::path::Path;
use std::sync::Mutex;

use tantivy::collector::{Count, TopDocs};
use tantivy::directory::MmapDirectory;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{IndexRecordOption, TantivyDocument, Value};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyError, Term};

use crate::models::entry::Entry;
use crate::repository::Repository;
use crate::repository::traits::ListEntriesParams;
use schema::EntryFields;

/// IndexWriter heap budget. 50 MB is Tantivy's recommended floor.
const WRITER_HEAP_BYTES: usize = 50_000_000;
/// Page size used when scanning the database during a full rebuild.
const REBUILD_PAGE_SIZE: i64 = 500;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("search index error: {0}")]
    Tantivy(#[from] TantivyError),
    #[error("open index error: {0}")]
    OpenDirectory(#[from] tantivy::directory::error::OpenDirectoryError),
    #[error("query parse error: {0}")]
    QueryParse(#[from] tantivy::query::QueryParserError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("repository error: {0}")]
    Repository(String),
    #[error("queue db error: {0}")]
    Db(String),
    #[error("search index is read-only in this process")]
    ReadOnly,
}

/// Filters + pagination for a search query. Mirrors the relevant subset of
/// `ListEntriesParams` (with `collection_slug` already resolved to an id).
pub struct SearchParams<'a> {
    pub site_id: &'a str,
    pub collection_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub published_only: bool,
    pub query: &'a str,
    pub page: i64,
    pub per_page: i64,
}

/// Ranked entry ids matching a query, plus the total number of matches.
pub struct SearchHits {
    pub ids: Vec<String>,
    pub total: usize,
}

/// Embedded full-text search engine for entries.
///
/// Read-write when opened with [`open`](Self::open) (the running server), read-only
/// when opened with [`open_read_only`](Self::open_read_only) (e.g. `cms mcp stdio`).
/// Read-only instances can [`search_entries`](Self::search_entries) but return
/// [`SearchError::ReadOnly`] from any write/commit/rebuild call.
pub struct SearchService {
    index: Index,
    reader: IndexReader,
    /// `Some` only for the writer-owning process; `None` for read-only openers.
    writer: Option<Mutex<IndexWriter>>,
    fields: EntryFields,
}

impl SearchService {
    /// Open (or create) the index read-write, acquiring Tantivy's directory lock.
    /// Only one process may hold this at a time — the running server.
    pub fn open(index_path: &Path) -> Result<Self, SearchError> {
        std::fs::create_dir_all(index_path)?;
        let (schema, fields) = schema::build_schema();
        let dir = MmapDirectory::open(index_path)?;
        let index = Index::open_or_create(dir, schema)?;
        schema::register_tokenizers(&index);

        let writer = index.writer(WRITER_HEAP_BYTES)?;
        // Manual reload: the sole writer reloads explicitly after each commit.
        let reader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into()?;

        Ok(Self {
            index,
            reader,
            writer: Some(Mutex::new(writer)),
            fields,
        })
    }

    /// Open the index read-only (no writer, no directory lock). Fails if the index
    /// does not exist yet. Used by processes that only search — they can run
    /// concurrently with the writer-owning server. The reader auto-reloads on the
    /// server's commits so results stay fresh.
    pub fn open_read_only(index_path: &Path) -> Result<Self, SearchError> {
        let dir = MmapDirectory::open(index_path)?;
        let index = Index::open(dir)?;
        schema::register_tokenizers(&index);
        let fields = schema::fields_from(&index.schema())?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            writer: None,
            fields,
        })
    }

    /// Whether the index currently holds no documents (used to trigger a rebuild).
    pub fn is_empty(&self) -> bool {
        self.reader.searcher().num_docs() == 0
    }

    /// Lock the writer, or fail if this instance is read-only.
    fn writer_guard(&self) -> Result<std::sync::MutexGuard<'_, IndexWriter>, SearchError> {
        self.writer
            .as_ref()
            .ok_or(SearchError::ReadOnly)
            .map(|m| m.lock().expect("search writer poisoned"))
    }

    /// Stage an upsert of one entry without committing (the indexer batches commits).
    pub fn index_doc(&self, entry: &Entry) -> Result<(), SearchError> {
        let writer = self.writer_guard()?;
        add_entry_doc(&writer, &self.fields, entry)
    }

    /// Stage a delete of one entry by id without committing.
    pub fn delete_doc(&self, id: &str) -> Result<(), SearchError> {
        let writer = self.writer_guard()?;
        writer.delete_term(Term::from_field_text(self.fields.id, id));
        Ok(())
    }

    /// Commit staged changes and reload the reader so searches see them.
    pub fn commit(&self) -> Result<(), SearchError> {
        self.commit_reload()
    }

    /// Run a ranked query with exact-match filters and pagination.
    pub fn search_entries(&self, params: &SearchParams<'_>) -> Result<SearchHits, SearchError> {
        let searcher = self.reader.searcher();

        let mut parser = QueryParser::for_index(&self.index, vec![self.fields.body, self.fields.slug]);
        // Require every user term (AND). Ranking is BM25 over the stemmed tokens,
        // so term frequency and field length drive relevance.
        parser.set_conjunction_by_default();

        let user_query = match parser.parse_query(params.query) {
            Ok(q) => q,
            // Reserved query syntax in user input — retry with a sanitized form.
            Err(_) => parser.parse_query(&sanitize_query(params.query))?,
        };

        let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![
            (Occur::Must, user_query),
            (Occur::Must, term_query(self.fields.site_id, params.site_id)),
        ];
        if let Some(cid) = params.collection_id {
            clauses.push((Occur::Must, term_query(self.fields.collection_id, cid)));
        }
        // `published_only` is the stricter constraint; otherwise honor an explicit status.
        let status_filter = if params.published_only {
            Some("published")
        } else {
            params.status
        };
        if let Some(status) = status_filter {
            clauses.push((Occur::Must, term_query(self.fields.status, status)));
        }
        let query = BooleanQuery::new(clauses);

        let per_page = params.per_page.max(1) as usize;
        let offset = ((params.page.max(1) - 1) * params.per_page.max(1)) as usize;
        let top_docs = TopDocs::with_limit(per_page).and_offset(offset).order_by_score();
        let (top, total) = searcher.search(&query, &(top_docs, Count))?;

        let mut ids = Vec::with_capacity(per_page);
        for (_score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            if let Some(id) = doc.get_first(self.fields.id).and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }
        }
        Ok(SearchHits { ids, total })
    }

    /// Drop every document and reindex all entries across all sites from the DB.
    pub async fn rebuild_all(&self, repo: &Repository) -> Result<usize, SearchError> {
        self.clear()?;
        let sites = repo
            .site
            .list_all()
            .await
            .map_err(|e| SearchError::Repository(e.to_string()))?;
        let mut count = 0;
        for site in &sites {
            count += self.index_site_uncommitted(repo, &site.id).await?;
        }
        self.commit_reload()?;
        Ok(count)
    }

    /// Reindex a single site: delete its documents, then reindex from the DB.
    pub async fn rebuild_site(&self, repo: &Repository, site_id: &str) -> Result<usize, SearchError> {
        {
            let writer = self.writer_guard()?;
            writer.delete_term(Term::from_field_text(self.fields.site_id, site_id));
        }
        self.commit_reload()?;
        let count = self.index_site_uncommitted(repo, site_id).await?;
        self.commit_reload()?;
        Ok(count)
    }

    /// Add every entry for a site to the writer without committing. Reuses the
    /// existing paginated `EntryRepository::list` (which includes singleton
    /// entries via the collection join), so no new repository method is needed.
    async fn index_site_uncommitted(&self, repo: &Repository, site_id: &str) -> Result<usize, SearchError> {
        let mut page = 1i64;
        let mut total = 0usize;
        loop {
            let result = repo
                .entry
                .list(ListEntriesParams {
                    site_id,
                    collection_slug: None,
                    collection_id: None,
                    status: None,
                    search: None,
                    published_only: false,
                    page,
                    per_page: REBUILD_PAGE_SIZE,
                })
                .await
                .map_err(|e| SearchError::Repository(e.to_string()))?;

            let batch = result.items.len();
            if batch == 0 {
                break;
            }
            {
                let writer = self.writer_guard()?;
                for entry in &result.items {
                    add_entry_doc(&writer, &self.fields, entry)?;
                }
            }
            total += batch;
            if (batch as i64) < REBUILD_PAGE_SIZE {
                break;
            }
            page += 1;
        }
        Ok(total)
    }

    fn clear(&self) -> Result<(), SearchError> {
        self.writer_guard()?.delete_all_documents()?;
        self.commit_reload()
    }

    fn commit_reload(&self) -> Result<(), SearchError> {
        self.writer_guard()?.commit()?;
        self.reader.reload()?;
        Ok(())
    }
}

fn term_query(field: tantivy::schema::Field, value: &str) -> Box<dyn Query> {
    Box::new(TermQuery::new(
        Term::from_field_text(field, value),
        IndexRecordOption::Basic,
    ))
}

/// Upsert one entry document: delete any prior version by id, then add the fresh
/// one. Does not commit — callers batch commits.
fn add_entry_doc(writer: &IndexWriter, fields: &EntryFields, entry: &Entry) -> Result<(), SearchError> {
    writer.delete_term(Term::from_field_text(fields.id, &entry.id));

    let mut doc = TantivyDocument::default();
    doc.add_text(fields.id, &entry.id);
    doc.add_text(fields.site_id, &entry.site_id);
    doc.add_text(fields.collection_id, &entry.collection_id);
    doc.add_text(fields.status, &entry.status);
    doc.add_text(fields.slug, &entry.slug);
    doc.add_text(fields.body, flatten_json_text(&entry.data));
    writer.add_document(doc)?;
    Ok(())
}

/// Concatenate every scalar (string/number/bool) leaf value in a JSON blob into a
/// single space-separated string for indexing. Object keys and JSON punctuation
/// are intentionally excluded so search matches content, not structure.
fn flatten_json_text(data: &str) -> String {
    let mut out = String::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
        collect_text(&value, &mut out);
    }
    out
}

fn collect_text(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(s) => {
            out.push_str(s);
            out.push(' ');
        }
        serde_json::Value::Number(n) => {
            out.push_str(&n.to_string());
            out.push(' ');
        }
        serde_json::Value::Bool(b) => {
            out.push_str(if *b { "true" } else { "false" });
            out.push(' ');
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_text(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_text(v, out);
            }
        }
        serde_json::Value::Null => {}
    }
}

/// Strip characters with special meaning to the query parser, leaving a plain
/// bag of words. Used only as a fallback when the raw query fails to parse.
fn sanitize_query(query: &str) -> String {
    query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::entry::Entry;

    fn entry(id: &str, site: &str, collection: &str, status: &str, slug: &str, data: &str) -> Entry {
        Entry {
            id: id.to_string(),
            site_id: site.to_string(),
            collection_id: collection.to_string(),
            data: data.to_string(),
            slug: slug.to_string(),
            status: status.to_string(),
            singleton_collection_id: None,
            created_at: "2026-01-01 00:00:00".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
            published_at: None,
        }
    }

    fn service() -> (SearchService, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let svc = SearchService::open(dir.path()).expect("open index");
        (svc, dir)
    }

    /// Stage + commit one entry (the indexer normally batches these).
    fn put(svc: &SearchService, e: &Entry) {
        svc.index_doc(e).unwrap();
        svc.commit().unwrap();
    }

    fn del(svc: &SearchService, id: &str) {
        svc.delete_doc(id).unwrap();
        svc.commit().unwrap();
    }

    fn search(svc: &SearchService, site: &str, query: &str) -> Vec<String> {
        svc.search_entries(&SearchParams {
            site_id: site,
            collection_id: None,
            status: None,
            published_only: false,
            query,
            page: 1,
            per_page: 20,
        })
        .expect("search")
        .ids
    }

    #[test]
    fn stems_and_finds_words() {
        let (svc, _d) = service();
        put(
            &svc,
            &entry("e1", "s1", "c1", "draft", "post", r#"{"title":"Running fast"}"#),
        );
        // "run" should match the stemmed "running".
        assert_eq!(search(&svc, "s1", "run"), vec!["e1"]);
    }

    #[test]
    fn isolates_by_site() {
        let (svc, _d) = service();
        put(
            &svc,
            &entry("e1", "s1", "c1", "draft", "a", r#"{"title":"shared word"}"#),
        );
        put(
            &svc,
            &entry("e2", "s2", "c1", "draft", "b", r#"{"title":"shared word"}"#),
        );
        assert_eq!(search(&svc, "s1", "shared"), vec!["e1"]);
        assert_eq!(search(&svc, "s2", "shared"), vec!["e2"]);
    }

    #[test]
    fn ranks_frequent_term_higher() {
        let (svc, _d) = service();
        put(
            &svc,
            &entry("low", "s1", "c1", "draft", "a", r#"{"body":"alpha once"}"#),
        );
        put(
            &svc,
            &entry("high", "s1", "c1", "draft", "b", r#"{"body":"alpha alpha alpha"}"#),
        );
        let ids = search(&svc, "s1", "alpha");
        assert_eq!(ids.first().map(String::as_str), Some("high"));
    }

    #[test]
    fn delete_removes_hit() {
        let (svc, _d) = service();
        put(&svc, &entry("e1", "s1", "c1", "draft", "a", r#"{"title":"gone"}"#));
        assert_eq!(search(&svc, "s1", "gone"), vec!["e1"]);
        del(&svc, "e1");
        assert!(search(&svc, "s1", "gone").is_empty());
    }

    #[test]
    fn upsert_replaces_content() {
        let (svc, _d) = service();
        put(&svc, &entry("e1", "s1", "c1", "draft", "a", r#"{"title":"first"}"#));
        put(&svc, &entry("e1", "s1", "c1", "draft", "a", r#"{"title":"second"}"#));
        assert!(search(&svc, "s1", "first").is_empty());
        assert_eq!(search(&svc, "s1", "second"), vec!["e1"]);
    }

    #[test]
    fn read_only_cannot_write() {
        let dir = tempfile::tempdir().expect("temp dir");
        // Create the index first (read_only fails on a nonexistent index).
        {
            let svc = SearchService::open(dir.path()).unwrap();
            put(&svc, &entry("e1", "s1", "c1", "draft", "a", r#"{"title":"hello"}"#));
        }
        let ro = SearchService::open_read_only(dir.path()).unwrap();
        // Reads work…
        assert_eq!(search(&ro, "s1", "hello"), vec!["e1"]);
        // …writes are rejected.
        assert!(matches!(
            ro.index_doc(&entry("e2", "s1", "c1", "draft", "b", r#"{"title":"x"}"#)),
            Err(SearchError::ReadOnly)
        ));
    }

    #[test]
    fn status_filter_applies() {
        let (svc, _d) = service();
        put(&svc, &entry("draft1", "s1", "c1", "draft", "a", r#"{"title":"topic"}"#));
        put(
            &svc,
            &entry("pub1", "s1", "c1", "published", "b", r#"{"title":"topic"}"#),
        );
        let ids = svc
            .search_entries(&SearchParams {
                site_id: "s1",
                collection_id: None,
                status: None,
                published_only: true,
                query: "topic",
                page: 1,
                per_page: 20,
            })
            .unwrap()
            .ids;
        assert_eq!(ids, vec!["pub1"]);
    }
}
