// ~/ag/backend/src/api/helpers.rs  v1.0
// Shared helper functions extracted from mod.rs

use super::*;

const LOG_FILE_PREFIX: &str = "backend.log";

/// Generate a short request ID for correlation
pub(crate) fn generate_request_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

pub(crate) fn validate_chunk_request(req: &ChunkConfigCommitRequest) -> Result<(), String> {
    if req.min_size == 0 {
        return Err("min_size must be greater than 0".into());
    }
    if req.min_size > req.target_size {
        return Err("min_size cannot exceed target_size".into());
    }
    if req.target_size > req.max_size {
        return Err("target_size cannot exceed max_size".into());
    }
    if req.overlap >= req.target_size {
        return Err("overlap must be smaller than target_size".into());
    }
    if req.max_size == 0 {
        return Err("max_size must be greater than 0".into());
    }
    if req
        .semantic_similarity_threshold
        .map_or(false, |v| !(0.0..=1.0).contains(&v))
    {
        return Err("semantic_similarity_threshold must be between 0 and 1".into());
    }
    if let Some(ref stages) = req.pipeline_stages {
        let valid: std::collections::HashSet<&str> = ["lw", "sent", "sem"].iter().copied().collect();
        let tokens: Vec<&str> = stages.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        for token in &tokens {
            if !valid.contains(token) {
                return Err(format!("unknown pipeline stage '{}'; valid values: lw, sent, sem", token));
            }
        }
        let unique: std::collections::HashSet<&&str> = tokens.iter().collect();
        if unique.len() < 2 {
            return Err("pipeline_stages must include at least 2 distinct stages".into());
        }
    }
    Ok(())
}

pub(crate) fn validate_llm_request(req: &LlmConfigRequest) -> Result<(), String> {
    if !(0.0..=2.0).contains(&req.temperature) {
        return Err("temperature must be between 0 and 2".into());
    }
    if !(0.0..=1.0).contains(&req.top_p) {
        return Err("top_p must be between 0 and 1".into());
    }
    if req.top_k == 0 {
        return Err("top_k must be greater than 0".into());
    }
    if req.max_tokens == 0 {
        return Err("max_tokens must be greater than 0".into());
    }
    if req.repeat_penalty < 1.0 {
        return Err("repeat_penalty must be at least 1.0".into());
    }
    if !(0.0..=2.0).contains(&req.frequency_penalty) {
        return Err("frequency_penalty must be between 0 and 2".into());
    }
    if !(0.0..=2.0).contains(&req.presence_penalty) {
        return Err("presence_penalty must be between 0 and 2".into());
    }
    if !(0.0..=1.0).contains(&req.min_p) {
        return Err("min_p must be between 0 and 1".into());
    }
    if !(0.0..=1.0).contains(&req.typical_p) {
        return Err("typical_p must be between 0 and 1".into());
    }
    if !(0.0..=1.0).contains(&req.tfs_z) {
        return Err("tfs_z must be between 0 and 1".into());
    }
    if !(0..=2).contains(&req.mirostat) {
        return Err("mirostat must be 0, 1, or 2".into());
    }
    if !(0.0..=1.0).contains(&req.mirostat_eta) {
        return Err("mirostat_eta must be between 0 and 1".into());
    }
    if !(0.0..=10.0).contains(&req.mirostat_tau) {
        return Err("mirostat_tau must be between 0 and 10".into());
    }
    if req.repeat_last_n == 0 {
        return Err("repeat_last_n must be greater than 0".into());
    }
    Ok(())
}

static LAST_HEALTH_STATUS: std::sync::OnceLock<std::sync::Mutex<String>> =
    std::sync::OnceLock::new();

/// Write to status-specific log file
pub(crate) fn write_status_log(status: &str, reason: &str, is_change: bool) {
    use std::io::Write;

    // Get log directory
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.agentic-rag/logs", home)
    });

    // Create log directory if needed
    let _ = std::fs::create_dir_all(&log_dir);

    // Status to filename mapping
    let filename = match status {
        "healthy" => "status_healthy.log",
        "busy" => "status_busy.log",
        "degraded" => "status_degraded.log",
        "unhealthy" => "status_unhealthy.log",
        "offline" => "status_offline.log",
        "checking" => "status_checking.log",
        _ => "status_unknown.log",
    };

    let log_path = format!("{}/{}", log_dir, filename);

    // Format log entry
    let timestamp = chrono::Utc::now().to_rfc3339();
    let change_type = if is_change { "CHANGED" } else { "INIT" };
    let entry = format!(
        "[{}] [{}] {} | {}\n",
        timestamp, change_type, status, reason
    );

    // Append to status-specific log file
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = file.write_all(entry.as_bytes());
    }
}

pub(crate) fn log_status_change(new_status: &str, reason: &str) {
    let status_lock = LAST_HEALTH_STATUS.get_or_init(|| std::sync::Mutex::new(String::new()));
    let mut last_status = status_lock.lock().unwrap();

    if *last_status != new_status {
        let is_change = !last_status.is_empty();

        // Write to status-specific log file
        write_status_log(new_status, reason, is_change);

        // Also log to main log
        if is_change {
            warn!(
                "Health status changed: {} -> {} | Reason: {}",
                last_status, new_status, reason
            );
        } else {
            info!(
                "Health status initialized: {} | Reason: {}",
                new_status, reason
            );
        }
        *last_status = new_status.to_string();
    }
}

pub(crate) fn latest_log_file(log_dir: &Path) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !file_name.starts_with(LOG_FILE_PREFIX) {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let replace = newest
                .as_ref()
                .map(|(ts, _)| modified > *ts)
                .unwrap_or(true);
            if replace {
                newest = Some((modified, path));
            }
        }
    }
    newest.map(|(_, path)| path)
}

pub(crate) fn read_recent_lines(path: &Path, limit: usize) -> std::io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut buffer = VecDeque::with_capacity(limit);
    for line in reader.lines() {
        let line = line?;
        if buffer.len() == limit {
            buffer.pop_front();
        }
        buffer.push_back(line);
    }
    Ok(buffer.into_iter().collect())
}

pub(crate) fn parse_log_line(line: &str) -> LogEntry {
    let parsed = serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| match value {
            Value::Object(_) => Some(value),
            _ => None,
        });
    if let Some(value) = parsed {
        let timestamp = value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let level = value
            .get("level")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let target = value
            .get("target")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let fields = value.get("fields").cloned();
        let message = fields
            .as_ref()
            .and_then(|f| f.get("message"))
            .or_else(|| value.get("message"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        LogEntry {
            timestamp,
            level,
            target,
            message,
            raw: line.to_string(),
            fields,
        }
    } else {
        LogEntry {
            timestamp: None,
            level: None,
            target: None,
            message: None,
            raw: line.to_string(),
            fields: None,
        }
    }
}

pub(crate) fn require_admin(req: &HttpRequest, config: &ApiConfig) -> Result<(), Error> {
    if let Some(expected) = &config.admin_api_token {
        if expected.is_empty() {
            return Ok(());
        }
        if let Some(header) = req.headers().get(AUTHORIZATION) {
            if let Ok(value) = header.to_str() {
                if value == expected || value.trim_start_matches("Bearer ") == expected {
                    return Ok(());
                }
            }
        }
        Err(actix_web::error::ErrorUnauthorized(
            "Missing or invalid admin API token",
        ))
    } else {
        Err(actix_web::error::ErrorUnauthorized(
            "ADMIN_API_TOKEN not configured",
        ))
    }
}

pub(crate) fn observe_manual_endpoint<F>(
    endpoint: &'static str,
    f: F,
) -> Result<HttpResponse, Error>
where
    F: FnOnce() -> Result<HttpResponse, Error>,
{
    let start = Instant::now();
    let span = info_span!("manual_observation", endpoint);
    let _guard = span.enter();
    let result = f();
    metrics::record_manual_observation(
        endpoint,
        result.is_ok(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    result
}

/// Helper for 3-layer memory search metrics (SEARCH.md)
/// layer: "search" | "timeline" | "fetch"
pub(crate) fn observe_memory_search_layer<F>(
    layer: &'static str,
    f: F,
) -> Result<HttpResponse, Error>
where
    F: FnOnce() -> Result<HttpResponse, Error>,
{
    let start = Instant::now();
    let span = info_span!("memory_search_layer", layer);
    let _guard = span.enter();
    let result = f();
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Record both the general manual observation metric and the layer-specific metric
    metrics::record_manual_observation(layer, result.is_ok(), duration_ms);
    metrics::record_memory_search_layer(layer, result.is_ok(), duration_ms);

    result
}
