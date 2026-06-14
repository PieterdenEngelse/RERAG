//! End-to-end roundtrip: the PointerRag-trigger slider on the frontend
//! is a thin wrapper around `PUT /runtime/settings/POINTERRAG_AUTO_GAP_THRESHOLD`.
//! The agent reads the same key via `settings::effective_f64`. This test
//! proves the chain:
//!
//! ```
//! HTTP PUT  →  Settings::set  →  Settings::effective_f64  →  auto_route()
//! ```
//!
//! Concretely: writing a value over HTTP must flip what `auto_route`
//! decides for the same `FragmentationStats`. If this passes, the slider
//! moves the routing decision; if it fails, the slider is decorative.
//!
//! Implementation note: `settings::install_global` is process-wide
//! (`OnceLock`), so the whole story lives in one `#[actix_web::test]`.
//! Splitting into multiple tests would race because they would share the
//! same global Settings store but run in parallel within the same test
//! binary.

use actix_web::{test, web, App};
use ag::agent::{auto_route, AutoRoute, FragmentationStats, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT};
use ag::api::runtime_routes;

const KEY: &str = "POINTERRAG_AUTO_GAP_THRESHOLD";

/// Fragmentation stats with `gap = section_ratio - doc_ratio = 0.60`.
/// Lets the test pick threshold values on either side of 0.60 to flip
/// the routing decision without changing any other input.
fn frag_gap_0_60() -> FragmentationStats {
    FragmentationStats {
        tracked: 5,
        untracked: 0,
        unique_sections: 4,
        unique_docs: 1,
        section_ratio: Some(0.80),
        doc_ratio: Some(0.20),
    }
}

#[actix_web::test]
async fn slider_writes_propagate_through_settings_into_auto_route() {
    // Fresh tmpdir overrides file so this test never collides with the
    // user's real ~/.local/share/ag/overrides.json.
    let tmp = tempfile::tempdir().expect("tmpdir");
    let overrides_path = tmp.path().join("overrides.json");

    // Make sure no stray env var hijacks the lookup (override → env → default).
    std::env::remove_var(KEY);

    // Install the global Settings store. Idempotent in case some earlier
    // test in this binary already installed (it shouldn't, but be safe).
    let settings = ag::settings::Settings::load(overrides_path.clone());
    let (_returned_path, recovery) =
        ag::settings::Recovery::boot_check(tmp.path(), &overrides_path);
    ag::settings::install_global(settings, std::sync::Arc::new(recovery));

    // Build an actix app with just the runtime-settings PUT route — the
    // exact surface the frontend slider hits via `put_runtime_setting`.
    let app = test::init_service(App::new().route(
        "/runtime/settings/{key}",
        web::put().to(runtime_routes::put_setting),
    ))
    .await;

    // Case 0 — baseline: no override yet → effective_f64 returns the
    // default the agent compiles in. Anchors the rest of the test.
    assert_eq!(
        ag::settings::effective_f64(KEY, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT),
        POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT,
        "with no override the agent should read the compiled default"
    );

    // Case 1 — slider pushed to 0.65 (Conservative): gap=0.60 falls
    // below, so auto_route falls through to Strict.
    let req = test::TestRequest::put()
        .uri(&format!("/runtime/settings/{KEY}"))
        .set_json(serde_json::json!({ "value": "0.65" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "PUT 0.65 should succeed, got {}",
        resp.status()
    );

    let threshold = ag::settings::effective_f64(KEY, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT);
    assert!(
        (threshold - 0.65).abs() < 1e-9,
        "effective_f64 should read back 0.65, got {threshold}"
    );

    let route = auto_route(frag_gap_0_60(), threshold, 5, 2000);
    assert_eq!(
        route,
        AutoRoute::Strict,
        "gap 0.60 < threshold 0.65 → Strict (high-confidence fallthrough)"
    );

    // Case 2 — slider pushed to 0.55 (Eager-ish): same fragmentation
    // input now meets the threshold, so auto_route flips to PointerHydration.
    // This is the load-bearing assertion: the slider value, written
    // over HTTP, actually changed where Auto routes.
    let req = test::TestRequest::put()
        .uri(&format!("/runtime/settings/{KEY}"))
        .set_json(serde_json::json!({ "value": "0.55" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "PUT 0.55 should succeed, got {}",
        resp.status()
    );

    let threshold = ag::settings::effective_f64(KEY, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT);
    assert!(
        (threshold - 0.55).abs() < 1e-9,
        "effective_f64 should read back 0.55, got {threshold}"
    );

    let route = auto_route(frag_gap_0_60(), threshold, 5, 2000);
    assert_eq!(
        route,
        AutoRoute::PointerHydration,
        "gap 0.60 ≥ threshold 0.55 → PointerHydration (slider moved the route)"
    );

    // Case 3 — invalid value: the registry kind is F64, so the handler
    // should reject "not_a_number" with 400 and leave the previous
    // override (0.55) untouched.
    let req = test::TestRequest::put()
        .uri(&format!("/runtime/settings/{KEY}"))
        .set_json(serde_json::json!({ "value": "not_a_number" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        400,
        "non-numeric value should be rejected by the F64 kind, got {}",
        resp.status()
    );

    let threshold = ag::settings::effective_f64(KEY, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT);
    assert!(
        (threshold - 0.55).abs() < 1e-9,
        "rejected write must not clobber the previous valid override, got {threshold}"
    );

    // Case 4 — clear: `{ "value": null }` removes the override and
    // effective_f64 returns to the compiled default. Same chain as
    // the slider's Reset button.
    let req = test::TestRequest::put()
        .uri(&format!("/runtime/settings/{KEY}"))
        .set_json(serde_json::json!({ "value": serde_json::Value::Null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "PUT null should succeed (clear), got {}",
        resp.status()
    );

    let threshold = ag::settings::effective_f64(KEY, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT);
    assert_eq!(
        threshold, POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT,
        "after clear, effective_f64 falls back to the compiled default"
    );
}
