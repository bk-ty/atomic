use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ==================== Tool Input Types ====================

/// Input parameters for semantic_search tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SemanticSearchParams {
    /// The search query to find relevant atoms using vector similarity
    pub query: String,

    /// Maximum number of results to return (default: 10, max: 50)
    #[serde(default)]
    pub limit: Option<i32>,

    /// Optional recency filter: only return atoms created within the last N days.
    /// Use this when the user asks about recent notes ("this week", "last month", etc.).
    #[serde(default)]
    pub since_days: Option<i32>,
}

/// Input parameters for read_atom tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadAtomParams {
    /// The UUID of the atom to retrieve
    pub atom_id: String,

    /// Maximum number of lines to return (default: 500, max: 500)
    #[serde(default)]
    pub limit: Option<i32>,

    /// Line offset for pagination, 0-indexed (default: 0)
    #[serde(default)]
    pub offset: Option<i32>,
}

/// Input parameters for create_atom tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateAtomParams {
    /// The markdown content of the atom
    pub content: String,

    /// Optional source URL where this content originated
    #[serde(default)]
    pub source_url: Option<String>,
}

/// Input parameters for update_atom tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateAtomParams {
    /// The UUID of the atom to update
    pub atom_id: String,

    /// Optional replacement markdown content for the atom. Omit to preserve current content.
    #[serde(default)]
    pub content: Option<String>,

    /// Optional replacement source URL. Omit to preserve current source URL.
    #[serde(default)]
    pub source_url: Option<String>,

    /// Optional replacement publication date. Omit to preserve current publication date.
    #[serde(default)]
    pub published_at: Option<String>,

    /// Optional replacement tag IDs. Omit to preserve current tags; pass [] to clear tags.
    #[serde(default)]
    pub tag_ids: Option<Vec<String>>,
}

/// A single edit operation for edit_atom.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditOperation {
    /// Operation type: replace, insert_after, append, or replace_all.
    pub operation: String,

    /// Exact text to replace. Required for replace and must occur exactly once.
    #[serde(default)]
    pub old_text: Option<String>,

    /// Replacement text for replace.
    #[serde(default)]
    pub new_text: Option<String>,

    /// Exact text to insert after. Required for insert_after and must occur exactly once.
    #[serde(default)]
    pub anchor_text: Option<String>,

    /// Text to insert for insert_after or append.
    #[serde(default)]
    pub text: Option<String>,

    /// Full replacement markdown content. Required for replace_all.
    #[serde(default)]
    pub content: Option<String>,
}

impl From<&EditOperation> for atomic_core::AtomEditOperation {
    fn from(value: &EditOperation) -> Self {
        Self {
            operation: value.operation.clone(),
            old_text: value.old_text.clone(),
            new_text: value.new_text.clone(),
            anchor_text: value.anchor_text.clone(),
            text: value.text.clone(),
            content: value.content.clone(),
        }
    }
}

/// Input parameters for edit_atom tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditAtomParams {
    /// The UUID of the atom to edit
    pub atom_id: String,

    /// Edits to apply in order. The whole operation fails if any edit is invalid.
    pub edits: Vec<EditOperation>,
}

/// Input parameters for ingest_url tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IngestUrlParams {
    /// URL to fetch, extract, and save as an atom. Exact source_url matches return the existing atom.
    pub url: String,
}

/// Input parameters for tag_health_report tool. All thresholds are
/// optional; defaults match the audit guidance from 2026-05-28
/// (avg=6.0, top_rate=0.5, single_child=5, p95=8.0).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TagHealthReportParams {
    /// Override avg_tags_per_atom regression threshold (default 6.0).
    pub avg_threshold: Option<f64>,
    /// Override top_tag_rate regression threshold (default 0.5).
    pub top_rate_threshold: Option<f64>,
    /// Override single_child_subtrees regression threshold (default 5).
    pub single_child_threshold: Option<i64>,
    /// Override p95 tags-per-atom regression threshold (default 8.0).
    pub p95_threshold: Option<f64>,
    /// Top-N tags scanned for 100%-overlap pair detection (default 50).
    pub overlap_top_n: Option<usize>,
    /// Atom-count below which a tag is counted as low-visibility (default 3).
    pub low_visibility_atom_count: Option<i32>,
}

impl Default for TagHealthReportParams {
    fn default() -> Self {
        Self {
            avg_threshold: None,
            top_rate_threshold: None,
            single_child_threshold: None,
            p95_threshold: None,
            overlap_top_n: None,
            low_visibility_atom_count: None,
        }
    }
}

// ==================== Tool Output Types ====================

/// A search result with atom content and similarity score
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub atom_id: String,
    pub content_preview: String,
    pub similarity_score: f32,
    pub matching_chunk: String,
}

/// Paginated atom content response
#[derive(Debug, Serialize)]
pub struct AtomContent {
    pub atom_id: String,
    pub content: String,
    pub total_lines: i32,
    pub returned_lines: i32,
    pub offset: i32,
    pub has_more: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Created/updated atom response
#[derive(Debug, Serialize)]
pub struct AtomResponse {
    pub atom_id: String,
    pub content_preview: String,
    pub tags: Vec<String>,
    pub embedding_status: String,
}

/// Ingested URL response
#[derive(Debug, Serialize)]
pub struct IngestUrlResponse {
    pub atom_id: String,
    pub url: String,
    pub title: String,
    pub content_length: usize,
    pub already_exists: bool,
}
