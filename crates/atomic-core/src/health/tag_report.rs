//! Structural tag-health metrics — drift-detection report.
//!
//! Distinct from the broader `health` module (which scores 11 categories of
//! data quality). This module focuses *only* on tag taxonomy hygiene:
//! - over-tagging drift (avg / p95 / max tags per atom),
//! - top-tag dominance (Production-Incidents-style junk drawers),
//! - single-child subtrees (premature hierarchy),
//! - 100% atom-set overlap pairs (redundant tags),
//! - parent-direct ratio (parent applied where a child should be),
//! - low-visibility / never-used tag accumulation.
//!
//! The report is read-only and cheap enough to schedule hourly. Used by:
//! 1. `GET /api/tagging/health-report` admin endpoint,
//! 2. `tag_health_report` MCP tool,
//! 3. `scripts/weekly-tag-health.sh` (markdown summary).
//!
//! Postgres backend is not yet supported (mirrors the rest of the health
//! module).

use crate::error::AtomicCoreError;
use crate::AtomicCore;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// Tunable thresholds for the regression detector. Values mirror the
/// 2026-05-28 audit findings: anything beyond these = active drift.
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TagHealthThresholds {
    /// Mean tags per atom that triggers a regression. Over-tagging drift.
    pub avg_tags_per_atom_max: f64,
    /// Top single-tag share (top tag's atom_count / total atoms) that
    /// triggers a regression. Production-Incidents pattern.
    pub top_tag_rate_max: f64,
    /// Parents with exactly one child are nearly-always premature
    /// hierarchy. More than this many = regression.
    pub single_child_subtrees_max: i64,
    /// 95th-percentile tags-per-atom over atoms with at least one tag.
    /// Triggers when LLM has been overzealous on dense atoms.
    pub p95_tags_per_atom_max: f64,
    /// Number of top tags (ordered by atom_count) considered for
    /// 100%-overlap pair detection.
    pub overlap_top_n: usize,
    /// Tag is "low visibility" when its atom_count is strictly less than
    /// this value.
    pub low_visibility_atom_count: i32,
}

impl Default for TagHealthThresholds {
    fn default() -> Self {
        Self {
            avg_tags_per_atom_max: 6.0,
            top_tag_rate_max: 0.5,
            single_child_subtrees_max: 5,
            p95_tags_per_atom_max: 8.0,
            overlap_top_n: 50,
            low_visibility_atom_count: 3,
        }
    }
}

#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopTagEntry {
    pub id: String,
    pub name: String,
    pub atom_count: i32,
    /// Share of corpus this tag covers (atom_count / total_atoms).
    pub rate: f64,
}

#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleChildSubtree {
    pub parent_id: String,
    pub parent_name: String,
    pub only_child_id: String,
    pub only_child_name: String,
}

#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentDirectRatioEntry {
    pub tag_id: String,
    pub tag_name: String,
    /// Distinct atoms tagged DIRECTLY with this tag (not via descendants).
    pub direct_atoms: i32,
    /// Distinct atoms in this tag's subtree (tag ∪ descendants).
    pub subtree_atoms: i32,
    /// direct / subtree. 1.0 means parent absorbs everything (junk drawer).
    pub ratio: f64,
}

#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlapPair {
    pub tag_a_id: String,
    pub tag_a_name: String,
    pub tag_b_id: String,
    pub tag_b_name: String,
    /// Number of atoms common to both (also equals each tag's atom_count
    /// when overlap is 100%).
    pub atom_count: i32,
}

#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    /// Stable check id, e.g. `avg_tags_per_atom`.
    pub check: String,
    /// Observed value (formatted to 4 decimals when float).
    pub value: serde_json::Value,
    /// Threshold that was breached.
    pub threshold: serde_json::Value,
    /// `info` | `warn` | `critical`.
    pub severity: String,
    /// Free-form human-readable detail for surfacing in the markdown
    /// summary or admin dashboard.
    pub message: String,
}

#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagHealthReport {
    /// RFC3339 timestamp of when the report was computed.
    pub computed_at: String,
    pub total_atoms: i32,
    pub total_tags: i32,
    /// Atoms with at least one tag.
    pub tagged_atoms: i32,
    /// Sum of all atom_tags rows.
    pub total_tag_assignments: i64,
    pub avg_tags_per_atom: f64,
    pub median_tags_per_atom: f64,
    pub p95_tags_per_atom: f64,
    pub max_tags_per_atom: i32,
    /// `tag_count -> atoms-with-that-many-tags`. Sparse map; only buckets
    /// with > 0 atoms are present. Used for spotting bimodal distributions.
    pub tag_count_distribution: HashMap<i32, i32>,
    /// Top tag's share of corpus (atom_count / total_atoms).
    pub top_tag_rate: f64,
    /// Top 10 tags (by atom_count) with rate.
    pub top_tags: Vec<TopTagEntry>,
    pub single_child_subtrees: i64,
    pub single_child_examples: Vec<SingleChildSubtree>,
    /// Tags with ≥1 child where the parent absorbs ≥50% of the subtree's
    /// atoms. Sorted by ratio descending.
    pub junk_drawer_tags: Vec<ParentDirectRatioEntry>,
    /// 100% atom-set overlap pairs among top-N tags.
    pub hundred_pct_overlap_pairs: Vec<OverlapPair>,
    pub low_visibility_tags_count: i32,
    pub never_used_tags_count: i32,
    /// All threshold breaches. Empty when healthy.
    pub regressions: Vec<Regression>,
    /// Thresholds used (echoed for audit / diff).
    pub thresholds: TagHealthThresholds,
}

// ==================== Public entry points ====================

/// Compute the full tag-health report against the database backing `core`.
///
/// Read-only; safe to schedule frequently. Returns a `Configuration` error
/// on Postgres backends (not yet supported, mirroring the rest of the
/// `health` module).
pub async fn compute_tag_health_report(
    core: &AtomicCore,
    thresholds: Option<TagHealthThresholds>,
) -> Result<TagHealthReport, AtomicCoreError> {
    let thresholds = thresholds.unwrap_or_default();
    let sqlite = core.storage().as_sqlite().ok_or_else(|| {
        AtomicCoreError::Configuration(
            "tag_health_report is not yet supported with Postgres backend".to_string(),
        )
    })?;
    let sqlite = sqlite.clone();
    let thresholds_for_task = thresholds.clone();
    let raw = tokio::task::spawn_blocking(move || {
        let conn = sqlite.db.read_conn()?;
        gather_raw(&conn)
    })
    .await
    .map_err(|e| AtomicCoreError::DatabaseOperation(format!("join error: {e}")))??;
    Ok(compute_from_raw(raw, thresholds_for_task))
}

// ==================== Raw data ====================

/// All data needed to compute the report, materialised once per call.
/// Public so tests in this module can build synthetic fixtures without
/// touching the database.
#[derive(Debug, Clone, Default)]
pub struct TagHealthRawData {
    /// Total atoms in the database (regardless of tag status).
    pub total_atoms: i32,
    /// `(tag_id, tag_name, parent_id, atom_count, is_autotag_target)`.
    pub tags: Vec<TagRow>,
    /// `(atom_id, tag_id)` rows from `atom_tags`.
    pub atom_tag_pairs: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub atom_count: i32,
    pub is_autotag_target: bool,
}

fn gather_raw(conn: &Connection) -> Result<TagHealthRawData, AtomicCoreError> {
    let total_atoms: i32 =
        conn.query_row("SELECT COUNT(*) FROM atoms", [], |r| r.get(0))?;

    let mut tags = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, atom_count, is_autotag_target FROM tags",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let is_target_int: i32 = row.get(4)?;
            tags.push(TagRow {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                atom_count: row.get(3)?,
                is_autotag_target: is_target_int != 0,
            });
        }
    }

    let mut atom_tag_pairs = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT atom_id, tag_id FROM atom_tags")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            atom_tag_pairs.push((row.get(0)?, row.get(1)?));
        }
    }

    Ok(TagHealthRawData {
        total_atoms,
        tags,
        atom_tag_pairs,
    })
}

// ==================== Pure compute ====================

/// Pure transformation from raw data to the report. Exposed for tests.
pub fn compute_from_raw(
    raw: TagHealthRawData,
    thresholds: TagHealthThresholds,
) -> TagHealthReport {
    let total_atoms = raw.total_atoms;
    let total_tags = raw.tags.len() as i32;
    let total_tag_assignments = raw.atom_tag_pairs.len() as i64;

    // -- per-atom tag counts --
    let mut counts_per_atom: HashMap<&str, i32> = HashMap::new();
    for (atom_id, _tag_id) in &raw.atom_tag_pairs {
        *counts_per_atom.entry(atom_id.as_str()).or_insert(0) += 1;
    }
    let tagged_atoms = counts_per_atom.len() as i32;
    let mut counts_sorted: Vec<i32> = counts_per_atom.values().copied().collect();
    counts_sorted.sort_unstable();

    let avg_tags_per_atom = if total_atoms > 0 {
        total_tag_assignments as f64 / total_atoms as f64
    } else {
        0.0
    };
    let (median_tags_per_atom, p95_tags_per_atom, max_tags_per_atom) =
        quantiles(&counts_sorted);

    let mut tag_count_distribution: HashMap<i32, i32> = HashMap::new();
    for &c in &counts_sorted {
        *tag_count_distribution.entry(c).or_insert(0) += 1;
    }

    // -- top tags --
    let mut tags_by_count: Vec<&TagRow> = raw.tags.iter().collect();
    tags_by_count.sort_by(|a, b| {
        b.atom_count
            .cmp(&a.atom_count)
            .then_with(|| a.name.cmp(&b.name))
    });
    let top_n_overlap = thresholds.overlap_top_n.max(1);
    let top_tags: Vec<TopTagEntry> = tags_by_count
        .iter()
        .take(10)
        .map(|t| TopTagEntry {
            id: t.id.clone(),
            name: t.name.clone(),
            atom_count: t.atom_count,
            rate: rate_of(t.atom_count, total_atoms),
        })
        .collect();
    let top_tag_rate = top_tags.first().map(|t| t.rate).unwrap_or(0.0);

    // -- single-child subtrees --
    let mut children_of: HashMap<&str, Vec<&TagRow>> = HashMap::new();
    for t in &raw.tags {
        if let Some(p) = &t.parent_id {
            children_of.entry(p.as_str()).or_default().push(t);
        }
    }
    let by_id: HashMap<&str, &TagRow> =
        raw.tags.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut single_child_examples = Vec::new();
    let mut single_child_count: i64 = 0;
    let mut sc_keys: Vec<(&&str, &Vec<&TagRow>)> = children_of.iter().collect();
    // Stable order: by parent id so the report is deterministic.
    sc_keys.sort_by(|a, b| a.0.cmp(b.0));
    for (parent_id, children) in sc_keys {
        if children.len() != 1 {
            continue;
        }
        single_child_count += 1;
        if single_child_examples.len() < 25 {
            if let (Some(parent), Some(only_child)) =
                (by_id.get(*parent_id), children.first())
            {
                single_child_examples.push(SingleChildSubtree {
                    parent_id: parent.id.clone(),
                    parent_name: parent.name.clone(),
                    only_child_id: only_child.id.clone(),
                    only_child_name: only_child.name.clone(),
                });
            }
        }
    }

    // -- atoms-by-tag index --
    let mut atoms_by_tag: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (atom_id, tag_id) in &raw.atom_tag_pairs {
        atoms_by_tag
            .entry(tag_id.as_str())
            .or_default()
            .insert(atom_id.as_str());
    }

    // -- parent_direct_ratio: junk-drawer detection --
    let mut junk_drawer_tags = Vec::new();
    for t in &raw.tags {
        let Some(child_list) = children_of.get(t.id.as_str()) else {
            continue;
        };
        if child_list.is_empty() {
            continue;
        }
        let direct = atoms_by_tag
            .get(t.id.as_str())
            .map(|s| s.len() as i32)
            .unwrap_or(0);
        let descendants = collect_descendant_ids(t.id.as_str(), &children_of);
        let mut subtree_atoms: HashSet<&str> = HashSet::new();
        for tid in &descendants {
            if let Some(set) = atoms_by_tag.get(tid.as_str()) {
                subtree_atoms.extend(set.iter().copied());
            }
        }
        let subtree_count = subtree_atoms.len() as i32;
        if subtree_count == 0 {
            continue;
        }
        let ratio = direct as f64 / subtree_count as f64;
        if ratio >= 0.5 && direct > 0 {
            junk_drawer_tags.push(ParentDirectRatioEntry {
                tag_id: t.id.clone(),
                tag_name: t.name.clone(),
                direct_atoms: direct,
                subtree_atoms: subtree_count,
                ratio,
            });
        }
    }
    junk_drawer_tags.sort_by(|a, b| {
        b.ratio
            .partial_cmp(&a.ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.subtree_atoms.cmp(&a.subtree_atoms))
            .then_with(|| a.tag_name.cmp(&b.tag_name))
    });

    // -- 100% overlap pairs across top-N tags --
    let mut hundred_pct_overlap_pairs: Vec<OverlapPair> = Vec::new();
    let top_n: Vec<&TagRow> = tags_by_count
        .iter()
        .take(top_n_overlap)
        .copied()
        .collect();
    for i in 0..top_n.len() {
        let a = top_n[i];
        let Some(set_a) = atoms_by_tag.get(a.id.as_str()) else {
            continue;
        };
        if set_a.is_empty() {
            continue;
        }
        for b in top_n.iter().skip(i + 1) {
            let Some(set_b) = atoms_by_tag.get(b.id.as_str()) else {
                continue;
            };
            if set_a.len() != set_b.len() {
                continue;
            }
            if set_a == set_b {
                hundred_pct_overlap_pairs.push(OverlapPair {
                    tag_a_id: a.id.clone(),
                    tag_a_name: a.name.clone(),
                    tag_b_id: b.id.clone(),
                    tag_b_name: b.name.clone(),
                    atom_count: set_a.len() as i32,
                });
            }
        }
    }
    hundred_pct_overlap_pairs.sort_by(|x, y| {
        y.atom_count
            .cmp(&x.atom_count)
            .then_with(|| x.tag_a_name.cmp(&y.tag_a_name))
            .then_with(|| x.tag_b_name.cmp(&y.tag_b_name))
    });

    // -- low-visibility / never-used --
    let low_visibility_tags_count = raw
        .tags
        .iter()
        .filter(|t| t.atom_count < thresholds.low_visibility_atom_count)
        .count() as i32;
    let never_used_tags_count = raw
        .tags
        .iter()
        .filter(|t| t.atom_count == 0 && !t.is_autotag_target)
        .count() as i32;

    // -- regressions --
    let mut regressions = Vec::new();
    if avg_tags_per_atom > thresholds.avg_tags_per_atom_max {
        regressions.push(Regression {
            check: "avg_tags_per_atom".into(),
            value: float_value(avg_tags_per_atom),
            threshold: float_value(thresholds.avg_tags_per_atom_max),
            severity: "warn".into(),
            message: format!(
                "Average tags per atom is {:.2} (threshold {:.2}). Drift back toward over-tagging.",
                avg_tags_per_atom, thresholds.avg_tags_per_atom_max,
            ),
        });
    }
    if top_tag_rate > thresholds.top_tag_rate_max {
        let name = top_tags
            .first()
            .map(|t| t.name.as_str())
            .unwrap_or("(unknown)");
        regressions.push(Regression {
            check: "top_tag_rate".into(),
            value: float_value(top_tag_rate),
            threshold: float_value(thresholds.top_tag_rate_max),
            severity: "critical".into(),
            message: format!(
                "Tag '{}' covers {:.1}% of the corpus (threshold {:.0}%). Junk-drawer pattern.",
                name,
                top_tag_rate * 100.0,
                thresholds.top_tag_rate_max * 100.0,
            ),
        });
    }
    if single_child_count > thresholds.single_child_subtrees_max {
        regressions.push(Regression {
            check: "single_child_subtrees".into(),
            value: serde_json::json!(single_child_count),
            threshold: serde_json::json!(thresholds.single_child_subtrees_max),
            severity: "warn".into(),
            message: format!(
                "{} parent tags have exactly one child (threshold {}). Premature hierarchy.",
                single_child_count, thresholds.single_child_subtrees_max,
            ),
        });
    }
    if p95_tags_per_atom > thresholds.p95_tags_per_atom_max {
        regressions.push(Regression {
            check: "p95_tags_per_atom".into(),
            value: float_value(p95_tags_per_atom),
            threshold: float_value(thresholds.p95_tags_per_atom_max),
            severity: "warn".into(),
            message: format!(
                "p95 tags-per-atom is {:.1} (threshold {:.1}). Long tail of over-tagged atoms.",
                p95_tags_per_atom, thresholds.p95_tags_per_atom_max,
            ),
        });
    }

    TagHealthReport {
        computed_at: chrono::Utc::now().to_rfc3339(),
        total_atoms,
        total_tags,
        tagged_atoms,
        total_tag_assignments,
        avg_tags_per_atom: round4(avg_tags_per_atom),
        median_tags_per_atom: round4(median_tags_per_atom),
        p95_tags_per_atom: round4(p95_tags_per_atom),
        max_tags_per_atom,
        tag_count_distribution,
        top_tag_rate: round4(top_tag_rate),
        top_tags,
        single_child_subtrees: single_child_count,
        single_child_examples,
        junk_drawer_tags,
        hundred_pct_overlap_pairs,
        low_visibility_tags_count,
        never_used_tags_count,
        regressions,
        thresholds,
    }
}

// ==================== Helpers ====================

fn quantiles(sorted: &[i32]) -> (f64, f64, i32) {
    if sorted.is_empty() {
        return (0.0, 0.0, 0);
    }
    let n = sorted.len();
    let median = if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        (sorted[n / 2 - 1] as f64 + sorted[n / 2] as f64) / 2.0
    };
    // Linear interpolation for p95 (matches NumPy's default).
    let p = 0.95;
    let pos = p * (n as f64 - 1.0);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let p95 = if lo == hi {
        sorted[lo] as f64
    } else {
        let frac = pos - lo as f64;
        sorted[lo] as f64 * (1.0 - frac) + sorted[hi] as f64 * frac
    };
    let max = *sorted.last().unwrap();
    (median, p95, max)
}

fn rate_of(numer: i32, denom: i32) -> f64 {
    if denom <= 0 {
        0.0
    } else {
        round4(numer as f64 / denom as f64)
    }
}

fn round4(x: f64) -> f64 {
    if x.is_finite() {
        (x * 10_000.0).round() / 10_000.0
    } else {
        x
    }
}

fn float_value(x: f64) -> serde_json::Value {
    serde_json::Number::from_f64(round4(x))
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null)
}

/// DFS collect of `root_id` plus all transitive descendants.
fn collect_descendant_ids(
    root_id: &str,
    children_of: &HashMap<&str, Vec<&TagRow>>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = vec![root_id.to_string()];
    let mut seen: HashSet<String> = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(kids) = children_of.get(id.as_str()) {
            for k in kids {
                stack.push(k.id.clone());
            }
        }
        out.push(id);
    }
    out
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(id: &str, name: &str, parent: Option<&str>, atom_count: i32) -> TagRow {
        TagRow {
            id: id.into(),
            name: name.into(),
            parent_id: parent.map(|s| s.into()),
            atom_count,
            is_autotag_target: false,
        }
    }

    fn pair(atom: &str, tag: &str) -> (String, String) {
        (atom.into(), tag.into())
    }

    fn defaults() -> TagHealthThresholds {
        TagHealthThresholds::default()
    }

    #[test]
    fn empty_report_is_sane() {
        let raw = TagHealthRawData::default();
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.total_atoms, 0);
        assert_eq!(report.total_tags, 0);
        assert_eq!(report.total_tag_assignments, 0);
        assert_eq!(report.tagged_atoms, 0);
        assert_eq!(report.avg_tags_per_atom, 0.0);
        assert_eq!(report.median_tags_per_atom, 0.0);
        assert_eq!(report.p95_tags_per_atom, 0.0);
        assert_eq!(report.max_tags_per_atom, 0);
        assert!(report.regressions.is_empty());
    }

    #[test]
    fn detects_avg_tags_per_atom_regression() {
        // 1 atom, 10 distinct tag assignments → avg = 10.
        let mut tags = Vec::new();
        let mut pairs = Vec::new();
        for i in 0..10 {
            tags.push(tag(
                &format!("t{}", i),
                &format!("Tag{}", i),
                None,
                1,
            ));
            pairs.push(pair("a1", &format!("t{}", i)));
        }
        let raw = TagHealthRawData {
            total_atoms: 1,
            tags,
            atom_tag_pairs: pairs,
        };
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.avg_tags_per_atom, 10.0);
        let avg = report
            .regressions
            .iter()
            .find(|r| r.check == "avg_tags_per_atom")
            .expect("avg_tags_per_atom regression must fire");
        assert_eq!(avg.severity, "warn");
        // p95 is also blown out at 10 → expect that regression too.
        assert!(report
            .regressions
            .iter()
            .any(|r| r.check == "p95_tags_per_atom"));
    }

    #[test]
    fn detects_top_tag_rate_regression() {
        // 10 atoms, all tagged with t1. top_tag_rate = 1.0 > 0.5.
        let tags = vec![tag("t1", "Production Incidents", None, 10)];
        let pairs: Vec<_> = (0..10).map(|i| pair(&format!("a{}", i), "t1")).collect();
        let raw = TagHealthRawData {
            total_atoms: 10,
            tags,
            atom_tag_pairs: pairs,
        };
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.top_tag_rate, 1.0);
        let r = report
            .regressions
            .iter()
            .find(|r| r.check == "top_tag_rate")
            .expect("top_tag_rate regression must fire");
        assert_eq!(r.severity, "critical");
        assert!(r.message.contains("Production Incidents"));
    }

    #[test]
    fn detects_single_child_subtree_regression() {
        // 6 parents each with exactly one child. Threshold = 5.
        let mut tags = Vec::new();
        for i in 0..6 {
            let parent = format!("p{}", i);
            let child = format!("c{}", i);
            tags.push(tag(&parent, &format!("Parent{}", i), None, 0));
            tags.push(tag(&child, &format!("Child{}", i), Some(&parent), 1));
        }
        let raw = TagHealthRawData {
            total_atoms: 1,
            tags,
            atom_tag_pairs: vec![pair("a1", "c0")],
        };
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.single_child_subtrees, 6);
        assert!(report
            .regressions
            .iter()
            .any(|r| r.check == "single_child_subtrees"));
        assert_eq!(report.single_child_examples.len(), 6);
    }

    #[test]
    fn detects_hundred_pct_overlap_pair() {
        // Two tags that share the exact same 3-atom set.
        let tags = vec![
            tag("t1", "Alpha", None, 3),
            tag("t2", "Beta", None, 3),
            tag("t3", "Gamma", None, 1), // distractor with different set
        ];
        let mut pairs = Vec::new();
        for atom in &["a1", "a2", "a3"] {
            pairs.push(pair(atom, "t1"));
            pairs.push(pair(atom, "t2"));
        }
        pairs.push(pair("a4", "t3"));
        let raw = TagHealthRawData {
            total_atoms: 4,
            tags,
            atom_tag_pairs: pairs,
        };
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.hundred_pct_overlap_pairs.len(), 1);
        let p = &report.hundred_pct_overlap_pairs[0];
        assert_eq!(p.atom_count, 3);
        let names = [&p.tag_a_name[..], &p.tag_b_name[..]];
        assert!(names.contains(&"Alpha"));
        assert!(names.contains(&"Beta"));
    }

    #[test]
    fn surfaces_junk_drawer_parent() {
        // Parent tag holds 8 direct atoms; its only child holds 2.
        // direct = 8, subtree = 10, ratio = 0.8 → junk drawer.
        let tags = vec![
            tag("p1", "Topics", None, 8),
            tag("c1", "Production Incidents", Some("p1"), 2),
        ];
        let mut pairs = Vec::new();
        for i in 0..8 {
            pairs.push(pair(&format!("a{}", i), "p1"));
        }
        for i in 8..10 {
            pairs.push(pair(&format!("a{}", i), "c1"));
        }
        let raw = TagHealthRawData {
            total_atoms: 10,
            tags,
            atom_tag_pairs: pairs,
        };
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.junk_drawer_tags.len(), 1);
        let entry = &report.junk_drawer_tags[0];
        assert_eq!(entry.tag_name, "Topics");
        assert_eq!(entry.direct_atoms, 8);
        assert_eq!(entry.subtree_atoms, 10);
        assert!((entry.ratio - 0.8).abs() < 1e-9);
    }

    #[test]
    fn quantiles_match_expected() {
        // 1, 2, 3, 4, 5 → median 3, p95 ≈ 4.8, max 5.
        let sorted = vec![1, 2, 3, 4, 5];
        let (median, p95, max) = quantiles(&sorted);
        assert_eq!(median, 3.0);
        assert!((p95 - 4.8).abs() < 1e-9);
        assert_eq!(max, 5);

        // Even-length: 1, 2, 3, 4 → median 2.5.
        let (median, _, _) = quantiles(&vec![1, 2, 3, 4]);
        assert_eq!(median, 2.5);
    }

    #[test]
    fn never_used_excludes_autotag_targets() {
        let mut t = tag("t1", "Topics", None, 0);
        t.is_autotag_target = true;
        let raw = TagHealthRawData {
            total_atoms: 0,
            tags: vec![t, tag("t2", "Stale Tag", None, 0)],
            atom_tag_pairs: vec![],
        };
        let report = compute_from_raw(raw, defaults());
        assert_eq!(report.never_used_tags_count, 1);
        assert_eq!(report.low_visibility_tags_count, 2);
    }

    #[test]
    fn healthy_corpus_emits_no_regressions() {
        // 10 atoms, two top-level tags splitting the corpus 5/5; each atom
        // tagged exactly once. avg = 1, top_tag_rate = 0.5 (not > 0.5),
        // p95 = 1, no single-child subtrees.
        let tags = vec![
            tag("t1", "Alpha", None, 5),
            tag("t2", "Beta", None, 5),
        ];
        let mut pairs = Vec::new();
        for i in 0..5 {
            pairs.push(pair(&format!("a{}", i), "t1"));
        }
        for i in 5..10 {
            pairs.push(pair(&format!("a{}", i), "t2"));
        }
        let raw = TagHealthRawData {
            total_atoms: 10,
            tags,
            atom_tag_pairs: pairs,
        };
        let report = compute_from_raw(raw, defaults());
        assert!(
            report.regressions.is_empty(),
            "expected no regressions, got {:?}",
            report.regressions
        );
        assert_eq!(report.top_tag_rate, 0.5);
    }
}
