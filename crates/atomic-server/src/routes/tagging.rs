//! Tagging-system routes (admin-only metrics).
//!
//! - GET `/api/tagging/health-report` — structural drift report (T46).
//!   Read-only, scoped to the database resolved by the `db` extractor.

use crate::db_extractor::Db;
use actix_web::{web, HttpResponse};
use atomic_core::health::TagHealthThresholds;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TagHealthQuery {
    /// Override the avg_tags_per_atom regression threshold.
    #[serde(default)]
    pub avg_threshold: Option<f64>,
    /// Override the top_tag_rate regression threshold.
    #[serde(default)]
    pub top_rate_threshold: Option<f64>,
    /// Override the single_child_subtrees regression threshold.
    #[serde(default)]
    pub single_child_threshold: Option<i64>,
    /// Override the p95 tags-per-atom regression threshold.
    #[serde(default)]
    pub p95_threshold: Option<f64>,
    /// Number of top tags considered for 100%-overlap pair detection.
    #[serde(default)]
    pub overlap_top_n: Option<usize>,
    /// Atom-count below which a tag is considered low-visibility.
    #[serde(default)]
    pub low_visibility_atom_count: Option<i32>,
}

impl TagHealthQuery {
    fn into_thresholds(self) -> TagHealthThresholds {
        let mut t = TagHealthThresholds::default();
        if let Some(v) = self.avg_threshold {
            t.avg_tags_per_atom_max = v;
        }
        if let Some(v) = self.top_rate_threshold {
            t.top_tag_rate_max = v;
        }
        if let Some(v) = self.single_child_threshold {
            t.single_child_subtrees_max = v;
        }
        if let Some(v) = self.p95_threshold {
            t.p95_tags_per_atom_max = v;
        }
        if let Some(v) = self.overlap_top_n {
            t.overlap_top_n = v.max(1);
        }
        if let Some(v) = self.low_visibility_atom_count {
            t.low_visibility_atom_count = v;
        }
        t
    }
}

#[utoipa::path(
    get,
    path = "/api/tagging/health-report",
    tag = "tagging",
    params(
        ("avg_threshold" = Option<f64>, Query, description = "Override avg_tags_per_atom regression threshold"),
        ("top_rate_threshold" = Option<f64>, Query, description = "Override top_tag_rate regression threshold"),
        ("single_child_threshold" = Option<i64>, Query, description = "Override single_child_subtrees regression threshold"),
        ("p95_threshold" = Option<f64>, Query, description = "Override p95 tags-per-atom regression threshold"),
        ("overlap_top_n" = Option<usize>, Query, description = "Top-N tags scanned for 100%-overlap pairs"),
        ("low_visibility_atom_count" = Option<i32>, Query, description = "Tag atom_count below which the tag is counted as low-visibility"),
    ),
    responses(
        (status = 200, description = "Tag-health report"),
        (status = 500, description = "Internal server error"),
    ),
    security(("bearer_auth" = [])),
)]
pub async fn get_tag_health_report(
    db: Db,
    query: web::Query<TagHealthQuery>,
) -> HttpResponse {
    let thresholds = query.into_inner().into_thresholds();
    match db.0.compute_tag_health_report(Some(thresholds)).await {
        Ok(report) => HttpResponse::Ok().json(report),
        Err(e) => crate::error::error_response(e),
    }
}
