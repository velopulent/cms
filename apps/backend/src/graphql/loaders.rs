use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::dataloader::Loader;

use crate::models::entry::Entry as DbEntry;
use crate::repository::Repository;

/// DataLoader key for batching `Collection.entry` resolution. Distinct
/// `(status, published_only)` combinations are grouped into one `IN (...)`
/// query each; in the common case (uniform filters) that is a single query for
/// the whole GraphQL request, eliminating the N+1.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EntriesByCollection {
    pub collection_id: String,
    pub status: Option<String>,
    pub published_only: bool,
}

pub struct EntryLoader {
    pub repository: Repository,
}

impl Loader<EntriesByCollection> for EntryLoader {
    type Value = Vec<DbEntry>;
    type Error = Arc<str>;

    async fn load(
        &self,
        keys: &[EntriesByCollection],
    ) -> Result<HashMap<EntriesByCollection, Self::Value>, Self::Error> {
        // Group collection ids by their (status, published_only) filter so each
        // group maps to exactly one batched query.
        let mut groups: HashMap<(Option<String>, bool), Vec<String>> = HashMap::new();
        for k in keys {
            groups
                .entry((k.status.clone(), k.published_only))
                .or_default()
                .push(k.collection_id.clone());
        }

        let mut out: HashMap<EntriesByCollection, Vec<DbEntry>> = HashMap::new();

        for ((status, published_only), ids) in groups {
            let entries = self
                .repository
                .entry
                .get_by_collection_ids(&ids, status.as_deref(), published_only)
                .await
                .map_err(|e| Arc::<str>::from(e.to_string()))?;

            let mut by_cid: HashMap<String, Vec<DbEntry>> = HashMap::new();
            for e in entries {
                by_cid.entry(e.collection_id.clone()).or_default().push(e);
            }

            for cid in ids {
                let key = EntriesByCollection {
                    collection_id: cid.clone(),
                    status: status.clone(),
                    published_only,
                };
                out.insert(key, by_cid.remove(&cid).unwrap_or_default());
            }
        }

        Ok(out)
    }
}
