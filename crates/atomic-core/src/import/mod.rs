//! Import utilities
//!
//! This module provides import functionality for various sources.

pub mod obsidian;

use serde::{Deserialize, Serialize};

/// Result of an import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: i32,
    pub updated: i32,
    pub skipped: i32,
    pub errors: i32,
    pub tags_created: i32,
    pub tags_linked: i32,
}

/// Progress event payload for import operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub current: i32,
    pub total: i32,
    pub current_file: String,
    pub status: String,
}

/// A folder source that has been imported. Stored in the `vaults` table so the
/// UI can surface a one-click re-sync without making the user re-pick the
/// folder every time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub id: i64,
    /// Display name (e.g. `ar-playbook`). Derived from the folder basename
    /// at first import; matches the `obsidian://<name>/...` source URL.
    pub name: String,
    /// Absolute filesystem path that was used at last import/sync.
    pub path: String,
    /// Source kind. Currently only `obsidian` is wired through this code path.
    pub kind: String,
    /// RFC3339 timestamp of the most recent successful sync, if any.
    pub last_synced_at: Option<String>,
    pub created_at: String,
    /// Number of atoms whose `source_url` matches this vault's namespace.
    /// Computed at list time; not stored.
    pub atom_count: i32,
    /// `false` iff `path` no longer exists on disk. Computed at list time.
    pub path_exists: bool,
}
