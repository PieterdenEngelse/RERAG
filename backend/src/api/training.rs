// ~/ag/backend/src/api/training.rs  v1.0
// Training data collection, LoRA export, synthetic QA generation

use super::*;

pub(crate) fn training_collector() -> &'static TrainingDataCollector {
    TRAINING_COLLECTOR.get_or_init(TrainingDataCollector::default)
}

pub(crate) fn lora_export_state() -> Arc<Mutex<LoraExportState>> {
    LORA_EXPORT_STATE
        .get_or_init(|| {
            Arc::new(Mutex::new(LoraExportState {
                running: false,
                last_started: None,
                last_finished: None,
                last_error: None,
            }))
        })
        .clone()
}

pub(crate) fn lora_filter_override() -> Arc<Mutex<Option<String>>> {
    LORA_FILTER_OVERRIDE
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

#[derive(Debug)]
pub(crate) struct LoraExportState {
    pub running: bool,
    pub last_started: Option<DateTime<Utc>>,
    pub last_finished: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SyntheticQaState {
    pub running: bool,
    pub last_started: Option<DateTime<Utc>>,
    pub last_finished: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub examples_generated: Option<usize>,
    pub questions_per_chunk: u32,
    pub max_chunks: Option<usize>,
}

pub(crate) fn synthetic_qa_state() -> Arc<Mutex<SyntheticQaState>> {
    SYNTHETIC_QA_STATE
        .get_or_init(|| Arc::new(Mutex::new(SyntheticQaState::default())))
        .clone()
}

#[derive(Debug, Default)]
pub(crate) struct AutoExportOverrides {
    pub auto_export_enabled: Option<bool>,
    pub debounce_ms: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct TrainingFeedbackRequest {
    pub query: String,
    pub response: String,
    pub context: Option<String>,
    pub quality_score: u8,
    pub conversation_id: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TrainingFeedbackResponse {
    pub status: String,
    pub example_id: String,
    pub message: String,
    pub request_id: String,
}

/// POST /training/feedback - Submit user feedback for training data collection
pub(crate) async fn submit_training_feedback(
    payload: web::Json<TrainingFeedbackRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();
    let collector = training_collector();

    if !collector.is_enabled() {
        return Ok(HttpResponse::Ok().json(TrainingFeedbackResponse {
            status: "skipped".into(),
            example_id: String::new(),
            message: "Training data collection is disabled".into(),
            request_id,
        }));
    }

    let example_id = uuid::Uuid::new_v4().to_string();
    let example = TrainingExample {
        id: example_id.clone(),
        instruction: body.query,
        context: body.context,
        response: body.response,
        quality_score: Some(body.quality_score.clamp(1, 5)),
        timestamp: chrono::Utc::now(),
        conversation_id: body.conversation_id,
        mode: body.mode,
        model: body.model,
    };

    match collector.add_example(example) {
        Ok(_) => {
            tracing::info!(
                example_id = %example_id,
                quality = body.quality_score,
                "Training feedback collected"
            );
            Ok(HttpResponse::Ok().json(TrainingFeedbackResponse {
                status: "collected".into(),
                example_id,
                message: "Thank you for your feedback!".into(),
                request_id,
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to collect training feedback");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save feedback: {}", e),
                "request_id": request_id
            })))
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct TrainingStatsResponse {
    pub status: String,
    pub request_id: String,
    pub stats: TrainingStats,
    pub collection_enabled: bool,
}

/// GET /training/stats - Get training data collection statistics
pub(crate) async fn get_training_stats() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let collector = training_collector();

    match collector.get_stats() {
        Ok(stats) => Ok(HttpResponse::Ok().json(TrainingStatsResponse {
            status: "ok".into(),
            request_id,
            stats,
            collection_enabled: collector.is_enabled(),
        })),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get training stats");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to get stats: {}", e),
                "request_id": request_id
            })))
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct TrainingExportResponse {
    pub status: String,
    pub request_id: String,
    pub exported_count: usize,
    pub output_path: String,
    pub message: String,
}

/// POST /training/export - Export collected data for Unsloth training
pub(crate) async fn export_training_data() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let collector = training_collector();

    // Determine export path
    let export_path = std::env::var("TRAINING_EXPORT_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("ag")
                .join("training")
                .join("training_data.jsonl")
        });

    // Ensure parent directory exists
    if let Some(parent) = export_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match collector.export_for_unsloth(&export_path) {
        Ok(count) => {
            tracing::info!(
                count = count,
                path = ?export_path,
                "Training data exported"
            );
            Ok(HttpResponse::Ok().json(TrainingExportResponse {
                status: "ok".into(),
                request_id,
                exported_count: count,
                output_path: export_path.to_string_lossy().to_string(),
                message: format!("Exported {} examples for Unsloth training", count),
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to export training data");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to export: {}", e),
                "request_id": request_id
            })))
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SnapshotExportResponse {
    pub status: String,
    pub request_id: String,
    pub message: String,
}

/// POST /training/export_snapshot - Run LoRA dataset export + normalization scripts
pub(crate) async fn export_lora_snapshot() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    match spawn_lora_export_job(true).await {
        Ok(()) => Ok(HttpResponse::Ok().json(SnapshotExportResponse {
            status: "ok".into(),
            request_id,
            message: "LoRA snapshot export started".into(),
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(SnapshotExportResponse {
                status: "error".into(),
                request_id,
                message: e,
            }),
        ),
    }
}

pub(crate) async fn spawn_lora_export_job(force: bool) -> Result<(), String> {
    use tokio::task;

    let state_handle = lora_export_state();

    {
        let mut state = state_handle
            .lock()
            .map_err(|_| "Failed to acquire export state".to_string())?;

        if state.running {
            if force {
                return Err("LoRA export already in progress".to_string());
            } else {
                return Err("running".to_string());
            }
        }

        state.running = true;
        state.last_started = Some(Utc::now());
        state.last_error = None;
    }

    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let scripts_dir = workspace_root
        .join("tools")
        .join("lora_training")
        .join("scripts");
    let export_script = scripts_dir.join("export_docs.py");
    let normalize_script = scripts_dir.join("normalize_dataset.py");
    let state_for_task = state_handle.clone();
    let filter = current_lora_filter();

    let job = task::spawn_blocking(move || {
        if let Some(ref value) = filter {
            std::env::set_var("LORA_EXPORT_ONLY", value);
        } else {
            std::env::remove_var("LORA_EXPORT_ONLY");
        }

        let result = run_script(&workspace_root, &export_script)
            .and_then(|_| run_script(&workspace_root, &normalize_script));

        let mut state = state_for_task.lock().expect("export state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        if let Err(ref err) = result {
            state.last_error = Some(err.clone());
        } else {
            state.last_error = None;
        }

        result
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Export snapshot task panicked");
        let mut state = state_handle.lock().expect("export state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        state.last_error = Some("task panicked".into());
        "Export task failed".to_string()
    })?;

    job
}

#[derive(Debug, Serialize)]
pub(crate) struct SnapshotStatusResponse {
    pub status: String,
    pub running: bool,
    pub last_started: Option<String>,
    pub last_finished: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SnapshotConfigResponse {
    pub status: String,
    pub auto_export_enabled: bool,
    pub default_debounce_ms: u64,
    pub export_filter: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateExportConfigRequest {
    pub auto_export_enabled: Option<bool>,
    pub default_debounce_ms: Option<u64>,
}

pub(crate) async fn export_snapshot_status() -> Result<HttpResponse, Error> {
    let state_handle = lora_export_state();
    let state = state_handle
        .lock()
        .map_err(|_| error::ErrorInternalServerError("Failed to acquire export state"))?;

    Ok(HttpResponse::Ok().json(SnapshotStatusResponse {
        status: "ok".into(),
        running: state.running,
        last_started: state.last_started.map(|dt| dt.to_rfc3339()),
        last_finished: state.last_finished.map(|dt| dt.to_rfc3339()),
        last_error: state.last_error.clone(),
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetExportFilterRequest {
    pub filter: Option<String>,
}

pub(crate) async fn set_export_snapshot_filter(
    payload: web::Json<SetExportFilterRequest>,
) -> Result<HttpResponse, Error> {
    if let Ok(mut guard) = lora_filter_override().lock() {
        *guard = payload.filter.clone();
    }
    export_snapshot_config().await
}

pub(crate) async fn export_snapshot_config() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(SnapshotConfigResponse {
        status: "ok".into(),
        auto_export_enabled: env_auto_export_enabled(),
        default_debounce_ms: env_auto_export_debounce_ms(),
        export_filter: current_lora_filter(),
    }))
}

pub(crate) async fn save_export_snapshot_config(
    payload: web::Json<UpdateExportConfigRequest>,
) -> Result<HttpResponse, Error> {
    let body = payload.into_inner();
    if let Some(enabled) = body.auto_export_enabled {
        set_auto_export_override(enabled);
    }
    if let Some(ms) = body.default_debounce_ms {
        set_auto_debounce_override(ms);
    }
    export_snapshot_config().await
}

pub(crate) fn auto_export_overrides() -> Arc<Mutex<AutoExportOverrides>> {
    AUTO_EXPORT_OVERRIDES
        .get_or_init(|| Arc::new(Mutex::new(AutoExportOverrides::default())))
        .clone()
}

pub(crate) fn set_auto_export_override(enabled: bool) {
    if let Ok(mut guard) = auto_export_overrides().lock() {
        guard.auto_export_enabled = Some(enabled);
    }
}

pub(crate) fn set_auto_debounce_override(ms: u64) {
    if let Ok(mut guard) = auto_export_overrides().lock() {
        guard.debounce_ms = Some(ms);
    }
}

pub(crate) fn env_auto_export_enabled() -> bool {
    if let Ok(guard) = auto_export_overrides().lock() {
        if let Some(value) = guard.auto_export_enabled {
            return value;
        }
    }
    std::env::var("AUTO_EXPORT_ON_UPLOAD")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(true)
}

pub(crate) fn env_auto_export_debounce_ms() -> u64 {
    if let Ok(guard) = auto_export_overrides().lock() {
        if let Some(value) = guard.debounce_ms {
            return value;
        }
    }
    std::env::var("AUTO_EXPORT_DEBOUNCE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}

pub(crate) fn env_lora_export_filter() -> Option<String> {
    std::env::var("LORA_EXPORT_ONLY").ok().and_then(|v| {
        if v.trim().is_empty() {
            None
        } else {
            Some(v)
        }
    })
}

pub(crate) fn current_lora_filter() -> Option<String> {
    if let Ok(guard) = lora_filter_override().lock() {
        if let Some(value) = guard.clone() {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    env_lora_export_filter()
}

pub(crate) fn trigger_auto_export_after_upload(upload_count: usize) {
    if upload_count == 0 || !env_auto_export_enabled() {
        return;
    }
    let debounce = env_auto_export_debounce_ms();
    tokio::spawn(async move {
        if debounce > 0 {
            sleep(Duration::from_millis(debounce)).await;
        }
        if let Err(err) = spawn_lora_export_job(false).await {
            tracing::warn!(error = %err, "Auto export skipped");
        }
    });
}

pub(crate) fn run_script(
    workspace_root: &std::path::Path,
    script_path: &std::path::Path,
) -> Result<(), String> {
    if !script_path.exists() {
        return Err(format!("Script not found: {}", script_path.display()));
    }

    let status = std::process::Command::new("python3")
        .arg(script_path)
        .current_dir(workspace_root)
        .status()
        .map_err(|e| format!("Failed to spawn {}: {}", script_path.display(), e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Script {} exited with status {:?}",
            script_path.display(),
            status.code()
        ))
    }
}

pub(crate) fn run_script_with_args(
    workspace_root: &std::path::Path,
    script_path: &std::path::Path,
    args: &[&str],
) -> Result<String, String> {
    if !script_path.exists() {
        return Err(format!("Script not found: {}", script_path.display()));
    }

    let output = std::process::Command::new("python3")
        .arg(script_path)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("Failed to spawn {}: {}", script_path.display(), e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Script {} failed: {}",
            script_path.display(),
            stderr
        ))
    }
}

// ============================================================================
// SYNTHETIC Q&A GENERATION
// ============================================================================

#[derive(Debug, Deserialize)]
pub(crate) struct SyntheticQaRequest {
    pub questions_per_chunk: Option<u32>,
    pub max_chunks: Option<usize>,
    pub ollama_model: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SyntheticQaResponse {
    pub status: String,
    pub request_id: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SyntheticQaStatusResponse {
    pub status: String,
    pub running: bool,
    pub last_started: Option<String>,
    pub last_finished: Option<String>,
    pub last_error: Option<String>,
    pub examples_generated: Option<usize>,
    pub questions_per_chunk: u32,
    pub max_chunks: Option<usize>,
}

/// POST /training/synthetic_qa - Generate synthetic Q&A training data
pub(crate) async fn generate_synthetic_qa(
    payload: Option<web::Json<SyntheticQaRequest>>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let (questions_per_chunk, max_chunks, ollama_model) = if let Some(p) = payload {
        (
            p.questions_per_chunk.unwrap_or(3),
            p.max_chunks,
            p.ollama_model.clone(),
        )
    } else {
        (3, None, None)
    };

    match spawn_synthetic_qa_job(questions_per_chunk, max_chunks, ollama_model).await {
        Ok(()) => Ok(HttpResponse::Ok().json(SyntheticQaResponse {
            status: "ok".into(),
            request_id,
            message: "Synthetic Q&A generation started".into(),
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(SyntheticQaResponse {
                status: "error".into(),
                request_id,
                message: e,
            }),
        ),
    }
}

pub(crate) async fn spawn_synthetic_qa_job(
    questions_per_chunk: u32,
    max_chunks: Option<usize>,
    ollama_model: Option<String>,
) -> Result<(), String> {
    use tokio::task;

    let state_handle = synthetic_qa_state();

    {
        let mut state = state_handle
            .lock()
            .map_err(|_| "Failed to acquire synthetic QA state".to_string())?;

        if state.running {
            return Err("Synthetic Q&A generation already in progress".to_string());
        }

        state.running = true;
        state.last_started = Some(Utc::now());
        state.last_error = None;
        state.examples_generated = None;
        state.questions_per_chunk = questions_per_chunk;
        state.max_chunks = max_chunks;
    }

    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let scripts_dir = workspace_root
        .join("tools")
        .join("lora_training")
        .join("scripts");
    let export_script = scripts_dir.join("export_docs.py");
    let synthetic_script = scripts_dir.join("generate_synthetic_qa.py");
    let state_for_task = state_handle.clone();
    let model = ollama_model.unwrap_or_else(|| "phi3.5:latest".to_string());

    let job = task::spawn_blocking(move || {
        // First run export_docs.py to ensure we have fresh document data
        tracing::info!("Running export_docs.py...");
        if let Err(e) = run_script(&workspace_root, &export_script) {
            return Err(format!("Export failed: {}", e));
        }

        // Build args for synthetic generation
        let mut args = vec![
            "--questions-per-chunk".to_string(),
            questions_per_chunk.to_string(),
            "--ollama-model".to_string(),
            model,
        ];

        if let Some(max) = max_chunks {
            args.push("--max-chunks".to_string());
            args.push(max.to_string());
        }

        tracing::info!(args = ?args, "Running generate_synthetic_qa.py...");

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let result = run_script_with_args(&workspace_root, &synthetic_script, &args_refs);

        // Parse output to get examples count
        let examples_count = if let Ok(ref output) = result {
            // Look for "Total examples: N" in output
            output
                .lines()
                .find(|line| line.contains("Total examples:"))
                .and_then(|line| {
                    line.split(':')
                        .nth(1)
                        .and_then(|s| s.trim().parse::<usize>().ok())
                })
        } else {
            None
        };

        let mut state = state_for_task.lock().expect("synthetic QA state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        state.examples_generated = examples_count;

        if let Err(ref err) = result {
            state.last_error = Some(err.clone());
        } else {
            state.last_error = None;
        }

        result.map(|_| ())
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Synthetic QA task panicked");
        let mut state = state_handle.lock().expect("synthetic QA state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        state.last_error = Some("task panicked".into());
        "Synthetic QA task failed".to_string()
    })?;

    job
}

/// GET /training/synthetic_qa/status - Get synthetic Q&A generation status
pub(crate) async fn synthetic_qa_status() -> Result<HttpResponse, Error> {
    let state_handle = synthetic_qa_state();
    let state = state_handle
        .lock()
        .map_err(|_| error::ErrorInternalServerError("Failed to acquire synthetic QA state"))?;

    Ok(HttpResponse::Ok().json(SyntheticQaStatusResponse {
        status: "ok".into(),
        running: state.running,
        last_started: state.last_started.map(|t| t.to_rfc3339()),
        last_finished: state.last_finished.map(|t| t.to_rfc3339()),
        last_error: state.last_error.clone(),
        examples_generated: state.examples_generated,
        questions_per_chunk: state.questions_per_chunk,
        max_chunks: state.max_chunks,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SyntheticQaExamplesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SyntheticQaExample {
    pub instruction: String,
    pub context: String,
    pub response: String,
    pub source: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SyntheticQaExamplesResponse {
    pub status: String,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub examples: Vec<SyntheticQaExample>,
}

/// GET /training/synthetic_qa/examples - Get generated synthetic Q&A examples
pub(crate) async fn synthetic_qa_examples(
    query: web::Query<SyntheticQaExamplesQuery>,
) -> Result<HttpResponse, Error> {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let qa_file = workspace_root
        .join("tools")
        .join("lora_training")
        .join("data")
        .join("synthetic_qa.jsonl");

    if !qa_file.exists() {
        return Ok(HttpResponse::Ok().json(SyntheticQaExamplesResponse {
            status: "ok".into(),
            total: 0,
            offset: 0,
            limit: query.limit.unwrap_or(10),
            examples: vec![],
        }));
    }

    let content = std::fs::read_to_string(&qa_file)
        .map_err(|e| error::ErrorInternalServerError(format!("Failed to read QA file: {}", e)))?;

    let all_examples: Vec<SyntheticQaExample> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .map(|v| SyntheticQaExample {
                    instruction: v
                        .get("instruction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    context: v
                        .get("context")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    response: v
                        .get("response")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source: v.get("source").and_then(|v| v.as_str()).map(String::from),
                    timestamp: v
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                })
        })
        .collect();

    let total = all_examples.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(10).min(100); // Cap at 100

    let examples: Vec<SyntheticQaExample> =
        all_examples.into_iter().skip(offset).take(limit).collect();

    Ok(HttpResponse::Ok().json(SyntheticQaExamplesResponse {
        status: "ok".into(),
        total,
        offset,
        limit,
        examples,
    }))
}

/// POST /training/clear - Clear all collected training data
pub(crate) async fn clear_training_data() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let collector = training_collector();

    match collector.clear() {
        Ok(_) => {
            tracing::info!("Training data cleared");
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "message": "Training data cleared",
                "request_id": request_id
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to clear training data");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to clear: {}", e),
                "request_id": request_id
            })))
        }
    }
}
