//! Tantivy schema for the entry search index.
//!
//! The index is a *derived* view of the `entries` table: the database remains the
//! source of truth and every document here can be rebuilt from it. We keep a small,
//! fixed schema regardless of the (schema-less) shape of `entries.data`:
//!
//! - `id`            — entry UUID; the primary term used to upsert/delete a document.
//! - `site_id`       — exact-match filter (multi-tenant isolation).
//! - `collection_id` — exact-match filter.
//! - `status`        — exact-match filter (`draft` / `published`).
//! - `slug`          — tokenized + stored, lightly searchable.
//! - `body`          — the flattened, tokenized text of all scalar values in
//!                     `entries.data`; the main ranked field.
//!
//! `body`/`slug` use an English-stemming analyzer so "running" matches "run".

use tantivy::Index;
use tantivy::schema::{IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer};

/// Name of the English-stemming tokenizer registered on the index.
pub const TOKENIZER_EN_STEM: &str = "en_stem";

/// Handles to every field in the entry schema, resolved once at open time.
#[derive(Clone)]
pub struct EntryFields {
    pub id: tantivy::schema::Field,
    pub site_id: tantivy::schema::Field,
    pub collection_id: tantivy::schema::Field,
    pub status: tantivy::schema::Field,
    pub slug: tantivy::schema::Field,
    pub body: tantivy::schema::Field,
}

/// Build the entry schema and return the resolved field handles alongside it.
pub fn build_schema() -> (Schema, EntryFields) {
    let mut builder = Schema::builder();

    let id = builder.add_text_field("id", STRING | STORED);
    let site_id = builder.add_text_field("site_id", STRING | STORED);
    let collection_id = builder.add_text_field("collection_id", STRING);
    let status = builder.add_text_field("status", STRING);

    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer(TOKENIZER_EN_STEM)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);

    let slug = builder.add_text_field(
        "slug",
        TextOptions::default()
            .set_indexing_options(text_indexing.clone())
            .set_stored(),
    );
    let body = builder.add_text_field("body", TextOptions::default().set_indexing_options(text_indexing));

    let schema = builder.build();
    (
        schema,
        EntryFields {
            id,
            site_id,
            collection_id,
            status,
            slug,
            body,
        },
    )
}

/// Resolve field handles from an already-open index's schema (used when opening
/// an existing index read-only, where we don't rebuild the schema ourselves).
pub fn fields_from(schema: &Schema) -> tantivy::Result<EntryFields> {
    Ok(EntryFields {
        id: schema.get_field("id")?,
        site_id: schema.get_field("site_id")?,
        collection_id: schema.get_field("collection_id")?,
        status: schema.get_field("status")?,
        slug: schema.get_field("slug")?,
        body: schema.get_field("body")?,
    })
}

/// Register the English-stemming tokenizer used by the `body`/`slug` fields.
pub fn register_tokenizers(index: &Index) {
    let en_stem = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(Stemmer::new(Language::English))
        .build();
    index.tokenizers().register(TOKENIZER_EN_STEM, en_stem);
}
