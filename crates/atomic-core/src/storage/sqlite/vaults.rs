//! Sqlite ops for the imported-vaults registry.
//!
//! Vaults are an emergent fact (one row per distinct folder source) that we
//! materialize into a table so the Settings UI can offer one-click re-sync
//! without making the user re-pick the folder every time.
//!
//! Counts and on-disk path existence are computed at list time, not stored,
//! to keep writes simple and avoid stale-cache bugs.

use crate::error::AtomicCoreError;
use crate::import::Vault;
use crate::storage::sqlite::SqliteStorage;
use crate::storage::traits::StorageResult;
use std::path::Path;

impl SqliteStorage {
    /// Upsert a vault by name. Updates `path` to the latest import folder so
    /// renames / moves on disk propagate automatically; never overwrites
    /// `created_at`. Returns the row id.
    pub(crate) fn upsert_vault_sync(
        &self,
        name: &str,
        path: &str,
        kind: &str,
        now: &str,
    ) -> StorageResult<i64> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        conn.execute(
            "INSERT INTO vaults (name, path, kind, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET
                 path = excluded.path,
                 kind = excluded.kind",
            rusqlite::params![name, path, kind, now],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM vaults WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// Mark a vault's `last_synced_at`. Called after both initial import and
    /// subsequent re-syncs.
    pub(crate) fn touch_vault_synced_sync(&self, id: i64, now: &str) -> StorageResult<()> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        conn.execute(
            "UPDATE vaults SET last_synced_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    }

    /// List all vaults with derived `atom_count` and `path_exists` fields.
    /// Atom count uses the `obsidian://<name>/` source-URL prefix as the
    /// vault namespace.
    pub(crate) fn list_vaults_sync(&self) -> StorageResult<Vec<Vault>> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT v.id, v.name, v.path, v.kind, v.last_synced_at, v.created_at,
                    (SELECT COUNT(*) FROM atoms a
                       WHERE a.source_url LIKE 'obsidian://' || v.name || '/%') AS atom_count
             FROM vaults v
             ORDER BY COALESCE(v.last_synced_at, v.created_at) DESC, v.name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let path: String = row.get(2)?;
                let path_exists = Path::new(&path).is_dir();
                Ok(Vault {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path,
                    kind: row.get(3)?,
                    last_synced_at: row.get(4)?,
                    created_at: row.get(5)?,
                    atom_count: row.get(6)?,
                    path_exists,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Fetch a single vault by id. Same derived fields as `list_vaults_sync`.
    pub(crate) fn get_vault_sync(&self, id: i64) -> StorageResult<Option<Vault>> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT v.id, v.name, v.path, v.kind, v.last_synced_at, v.created_at,
                    (SELECT COUNT(*) FROM atoms a
                       WHERE a.source_url LIKE 'obsidian://' || v.name || '/%') AS atom_count
             FROM vaults v
             WHERE v.id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            let path: String = row.get(2)?;
            let path_exists = Path::new(&path).is_dir();
            Ok(Vault {
                id: row.get(0)?,
                name: row.get(1)?,
                path,
                kind: row.get(3)?,
                last_synced_at: row.get(4)?,
                created_at: row.get(5)?,
                atom_count: row.get(6)?,
                path_exists,
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Update a vault's stored path. Used when the user re-binds a vault to
    /// a moved/renamed folder via the UI.
    pub(crate) fn update_vault_path_sync(&self, id: i64, path: &str) -> StorageResult<()> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        conn.execute(
            "UPDATE vaults SET path = ?1 WHERE id = ?2",
            rusqlite::params![path, id],
        )?;
        Ok(())
    }

    /// Remove a vault registry row. Atoms are NOT deleted; "Remove" only
    /// stops the vault from appearing in the sync UI. Cleanup of orphaned
    /// atoms is a separate, explicit user action by design.
    pub(crate) fn delete_vault_sync(&self, id: i64) -> StorageResult<()> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        conn.execute("DELETE FROM vaults WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }
}

impl SqliteStorage {
    /// Look up a vault by stored `path` (not the on-disk basename). Returns
    /// the row if exactly one match exists. Used at import time so that a
    /// re-bound folder reuses its registered name and source URLs stay
    /// stable across renames.
    pub(crate) fn get_vault_by_path_sync(
        &self,
        path: &str,
    ) -> StorageResult<Option<Vault>> {
        let conn = self
            .db
            .conn
            .lock()
            .map_err(|e| AtomicCoreError::Lock(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT v.id, v.name, v.path, v.kind, v.last_synced_at, v.created_at,
                    (SELECT COUNT(*) FROM atoms a
                       WHERE a.source_url LIKE 'obsidian://' || v.name || '/%') AS atom_count
             FROM vaults v
             WHERE v.path = ?1
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![path], |row| {
            let p: String = row.get(2)?;
            let path_exists = Path::new(&p).is_dir();
            Ok(Vault {
                id: row.get(0)?,
                name: row.get(1)?,
                path: p,
                kind: row.get(3)?,
                last_synced_at: row.get(4)?,
                created_at: row.get(5)?,
                atom_count: row.get(6)?,
                path_exists,
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }
}
