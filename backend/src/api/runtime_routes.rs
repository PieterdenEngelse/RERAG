//! HTTP routes for runtime settings + the universal self-restart action.

use actix_web::{web, HttpResponse, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SetOverrideBody {
    /// `None` clears the override (reverts to env/default); `Some` sets it.
    pub value: Option<String>,
}

/// GET /runtime/settings — full snapshot for the UI: every known key,
/// plus any unregistered keys that have overrides set, plus the most recent
/// rollback (if any).
pub async fn get_settings() -> Result<HttpResponse> {
    let snapshot = match crate::settings::global() {
        Some(s) => s.snapshot(),
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "settings store not initialized",
            })));
        }
    };
    let rollback = crate::settings::last_rollback();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "entries": snapshot.entries,
        "last_rollback": rollback,
    })))
}

/// PUT /runtime/settings/{key} — set or clear an override.
/// Body: `{ "value": "..." }` to set, `{ "value": null }` to clear.
pub async fn put_setting(
    path: web::Path<String>,
    body: web::Json<SetOverrideBody>,
) -> Result<HttpResponse> {
    let settings = match crate::settings::global() {
        Some(s) => s,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "settings store not initialized",
            })));
        }
    };
    let key = path.into_inner();
    let value = body.into_inner().value;
    match settings.set(&key, value) {
        Ok(()) => {
            let entry = settings
                .snapshot()
                .entries
                .into_iter()
                .find(|e| e.key == key);
            let restart_required = entry.as_ref().map(|e| e.restart_required).unwrap_or(false);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "ok": true,
                "key": key,
                "restart_required": restart_required,
                "entry": entry,
            })))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "key": key,
            "error": e,
        }))),
    }
}

/// DELETE /runtime/settings/{key} — clear an override.
pub async fn delete_setting(path: web::Path<String>) -> Result<HttpResponse> {
    let settings = match crate::settings::global() {
        Some(s) => s,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "settings store not initialized",
            })));
        }
    };
    let key = path.into_inner();
    match settings.set(&key, None) {
        Ok(()) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "ok": true,
            "key": key,
        }))),
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": e,
        }))),
    }
}

/// GET /runtime/capabilities — what this deployment can do.
pub async fn get_capabilities() -> Result<HttpResponse> {
    match crate::capabilities::global() {
        Some(caps) => Ok(HttpResponse::Ok().json(&*caps)),
        None => Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "capabilities not detected yet",
        }))),
    }
}

/// POST /runtime/actions/restart-self — drain briefly, then re-exec.
/// Universal: works in bin, systemd, container, anywhere.
pub async fn post_restart_self() -> Result<HttpResponse> {
    // Spawn a delayed re-exec so we can still return the 202 to the caller.
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = crate::lifecycle::restart_self();
    });
    Ok(HttpResponse::Accepted().json(serde_json::json!({
        "ok": true,
        "message": "ag is restarting via self re-exec; the page will refresh shortly",
    })))
}
