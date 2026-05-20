//! Imported-vaults registry routes.

use crate::db_extractor::Db;
use crate::event_bridge::embedding_event_callback;
use crate::state::{AppState, ServerEvent};
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct RebindVaultRequest {
    /// New absolute filesystem path for this vault.
    pub path: String,
}

#[utoipa::path(get, path = "/api/vaults", responses((status = 200, description = "List imported vaults")), tag = "vaults")]
pub async fn list_vaults(db: Db) -> HttpResponse {
    match db.0.list_vaults().await {
        Ok(vaults) => HttpResponse::Ok().json(vaults),
        Err(e) => crate::error::error_response(e),
    }
}

#[utoipa::path(post, path = "/api/vaults/{id}/sync", responses((status = 200, description = "Sync result")), tag = "vaults")]
pub async fn sync_vault(
    state: web::Data<AppState>,
    db: Db,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();
    let on_event = embedding_event_callback(state.event_tx.clone());
    let tx = state.event_tx.clone();
    let on_progress = move |progress: atomic_core::ImportProgress| {
        let _ = tx.send(ServerEvent::ImportProgress {
            current: progress.current,
            total: progress.total,
            current_file: progress.current_file,
            status: progress.status,
        });
    };
    match db.0.sync_vault(id, on_event, on_progress).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => crate::error::error_response(e),
    }
}

#[utoipa::path(put, path = "/api/vaults/{id}", request_body = RebindVaultRequest, responses((status = 204, description = "Vault path updated")), tag = "vaults")]
pub async fn rebind_vault(
    db: Db,
    path: web::Path<i64>,
    body: web::Json<RebindVaultRequest>,
) -> HttpResponse {
    let id = path.into_inner();
    match db.0.rebind_vault(id, &body.path).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => crate::error::error_response(e),
    }
}

#[utoipa::path(delete, path = "/api/vaults/{id}", responses((status = 204, description = "Vault removed from registry")), tag = "vaults")]
pub async fn delete_vault(db: Db, path: web::Path<i64>) -> HttpResponse {
    let id = path.into_inner();
    match db.0.delete_vault(id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => crate::error::error_response(e),
    }
}
