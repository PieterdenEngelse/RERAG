use actix_web::{test, App};
use ag::api::agentic_monitor_routes;
use ag::monitoring::tool_trends;
use serde_json::from_slice;
use uuid::Uuid;

fn record_sample_executions(tool_name: &str, count: usize) {
    for idx in 0..count {
        let success = idx % 2 == 0;
        let latency_ms = 75 + (idx as u64 * 5);
        let confidence = 0.5 + (idx as f32 * 0.1);
        let cost = 0.001 * (idx as f64 + 1.0);
        tool_trends::record_execution(tool_name, success, latency_ms, confidence, cost);
    }
}

#[actix_web::test]
async fn tool_trends_defaults_to_hour_window() {
    let tool_name = format!("TestToolHour-{}", Uuid::new_v4());
    let sample_count = 3;
    record_sample_executions(&tool_name, sample_count);

    let app = test::init_service(
        App::new().configure(agentic_monitor_routes::configure_agentic_monitor_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/monitoring/tools/trends")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let payload: agentic_monitor_routes::ToolTrendsResponse = from_slice(&body).unwrap();

    assert_eq!(payload.window, "Hour");

    let trend = payload
        .trends
        .iter()
        .find(|trend| trend.tool_type == tool_name)
        .expect("expected trend data for recorded tool");

    assert_eq!(trend.window, "Hour");
    assert!(trend.summary.total_executions >= sample_count);
    assert!(trend.buckets.iter().any(|bucket| bucket.executions > 0));
}

#[actix_web::test]
async fn tool_trends_respects_window_query_parameter() {
    let tool_name = format!("TestToolDay-{}", Uuid::new_v4());
    let sample_count = 4;
    record_sample_executions(&tool_name, sample_count);

    let app = test::init_service(
        App::new().configure(agentic_monitor_routes::configure_agentic_monitor_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/monitoring/tools/trends?window=day")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let payload: agentic_monitor_routes::ToolTrendsResponse = from_slice(&body).unwrap();

    assert_eq!(payload.window, "Day");

    let trend = payload
        .trends
        .iter()
        .find(|trend| trend.tool_type == tool_name)
        .expect("expected trend data for recorded tool");

    assert_eq!(trend.window, "Day");
    assert!(trend.summary.total_executions >= sample_count);
}
