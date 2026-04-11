// ~/ag/backend/src/api/docker.rs  v1.0
// Docker monitoring, runtime actions, container management

use super::*;

// ============================================================================
// DOCKER MONITORING
// ============================================================================

/// Docker container info
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DockerContainer {
    name: String,
    image: String,
    status: String,
    state: String,
    ports: Vec<String>,
    created: String,
    health: Option<String>,
}

/// Docker stats for a container
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DockerStats {
    name: String,
    cpu_percent: f64,
    memory_usage: String,
    memory_limit: String,
    memory_percent: f64,
    network_rx: String,
    network_tx: String,
}

/// GET /monitoring/docker
/// Returns Docker container status and stats for ag infrastructure
pub(crate) async fn get_docker_status() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Try to get Docker container info by running docker commands
    // This runs docker ps and docker stats to get container info

    let containers = get_docker_containers().await;
    let stats = get_docker_stats().await;
    let docker_available = !containers.is_empty() || check_docker_available().await;

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "docker_available": docker_available,
        "containers": containers,
        "stats": stats
    })))
}

/// Check if Docker is available
pub(crate) async fn check_docker_available() -> bool {
    match tokio::process::Command::new("docker")
        .args(["info"])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .await
    {
        Ok(output) => {
            if output.status.success() {
                return true;
            }
            // Check if it's a permission error
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("permission denied") {
                warn!("Docker permission denied. Add user to docker group: sudo usermod -aG docker $USER");
            }
            false
        }
        Err(_) => false,
    }
}

/// Get Docker container list
pub(crate) async fn get_docker_containers() -> Vec<DockerContainer> {
    // Run: docker ps -a --filter "name=ag-" --format json
    let output = match tokio::process::Command::new("docker")
        .args(["ps", "-a", "--filter", "name=ag-", "--format", "{{json .}}"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run docker ps: {}", e);
            return Vec::new();
        }
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let name = json["Names"].as_str().unwrap_or("").to_string();
            let image = json["Image"].as_str().unwrap_or("").to_string();
            let status = json["Status"].as_str().unwrap_or("").to_string();
            let state = json["State"].as_str().unwrap_or("").to_string();
            let ports_str = json["Ports"].as_str().unwrap_or("");
            let created = json["CreatedAt"].as_str().unwrap_or("").to_string();

            // Parse ports
            let ports: Vec<String> = if ports_str.is_empty() {
                Vec::new()
            } else {
                ports_str
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            };

            // Extract health from status if present
            let health = if status.contains("(healthy)") {
                Some("healthy".to_string())
            } else if status.contains("(unhealthy)") {
                Some("unhealthy".to_string())
            } else if status.contains("(health: starting)") {
                Some("starting".to_string())
            } else {
                None
            };

            containers.push(DockerContainer {
                name,
                image,
                status,
                state,
                ports,
                created,
                health,
            });
        }
    }

    containers
}

/// Get Docker container stats
pub(crate) async fn get_docker_stats() -> Vec<DockerStats> {
    // Run: docker stats --no-stream --format json for ag containers
    let output = match tokio::process::Command::new("docker")
        .args(["stats", "--no-stream", "--format", "{{json .}}"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run docker stats: {}", e);
            return Vec::new();
        }
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut stats = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let name = json["Name"].as_str().unwrap_or("").to_string();

            // Only include ag containers
            if !name.starts_with("ag-") {
                continue;
            }

            let cpu_str = json["CPUPerc"].as_str().unwrap_or("0%");
            let cpu_percent = cpu_str.trim_end_matches('%').parse::<f64>().unwrap_or(0.0);

            let mem_usage = json["MemUsage"].as_str().unwrap_or("0B / 0B").to_string();
            let (memory_usage, memory_limit) = if mem_usage.contains(" / ") {
                let parts: Vec<&str> = mem_usage.split(" / ").collect();
                (
                    parts.get(0).unwrap_or(&"0B").to_string(),
                    parts.get(1).unwrap_or(&"0B").to_string(),
                )
            } else {
                (mem_usage.clone(), "0B".to_string())
            };

            let mem_perc_str = json["MemPerc"].as_str().unwrap_or("0%");
            let memory_percent = mem_perc_str
                .trim_end_matches('%')
                .parse::<f64>()
                .unwrap_or(0.0);

            let net_io = json["NetIO"].as_str().unwrap_or("0B / 0B").to_string();
            let (network_rx, network_tx) = if net_io.contains(" / ") {
                let parts: Vec<&str> = net_io.split(" / ").collect();
                (
                    parts.get(0).unwrap_or(&"0B").to_string(),
                    parts.get(1).unwrap_or(&"0B").to_string(),
                )
            } else {
                (net_io.clone(), "0B".to_string())
            };

            stats.push(DockerStats {
                name,
                cpu_percent,
                memory_usage,
                memory_limit,
                memory_percent,
                network_rx,
                network_tx,
            });
        }
    }

    stats
}

// ============================================================================
// RUNTIME ACTIONS (LLM runtime control)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RuntimeActionRequest {
    action: String,
}

/// POST /monitoring/runtime/action
/// Stop/start the LLM runtime (currently Ollama via systemd).
///
/// Notes:
/// - This requires the backend process user to have permission to run `systemctl` for ollama
///   without an interactive password prompt.
pub(crate) async fn runtime_action(
    body: web::Json<RuntimeActionRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let action = body.action.as_str();

    // Handle backend switching: stop one, start the other
    let commands: Vec<(&str, &str)> = match action {
        "stop" => vec![("stop", "ollama.service")],
        "start" => vec![("start", "ollama.service")],
        "switch_ollama" => vec![
            ("stop", "llama-server.service"),
            ("start", "ollama.service"),
        ],
        "switch_llama_cpp" => vec![
            ("stop", "ollama.service"),
            ("start", "llama-server.service"),
        ],
        _ => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Unknown runtime action: {}", action),
            })));
        }
    };

    // Execute commands sequentially
    for (cmd, service) in &commands {
        let output = tokio::process::Command::new("systemctl")
            .arg("--user")
            .args(&[*cmd, *service])
            .output()
            .await;

        if let Err(e) = output {
            tracing::warn!("Failed to {} {}: {}", cmd, service, e);
        }
    }

    // Give service time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Return current status
    let ollama_running = tokio::process::Command::new("systemctl")
        .args(&["--user", "is-active", "ollama.service"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let llama_running = tokio::process::Command::new("systemctl")
        .args(&["--user", "is-active", "llama-server.service"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let active_backend = if llama_running {
        "llama_cpp"
    } else if ollama_running {
        "ollama"
    } else {
        "none"
    };

    return Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "active_backend": active_backend,
        "ollama_running": ollama_running,
        "llama_cpp_running": llama_running,
    })));
}

#[allow(dead_code)]
async fn runtime_action_legacy(
    body: web::Json<RuntimeActionRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let action = body.action.as_str();

    let args: Vec<&str> = match action {
        "stop" => vec!["stop", "ollama.service"],
        "start" => vec!["start", "ollama.service"],
        _ => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Unknown runtime action: {}", action),
            })));
        }
    };

    let output = tokio::process::Command::new("systemctl")
        .arg("--user")
        .args(&args)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if out.status.success() {
                Ok(HttpResponse::Ok().json(json!({
                    "status": "ok",
                    "request_id": request_id,
                    "action": action,
                    "stdout": stdout,
                    "stderr": stderr,
                })))
            } else {
                Ok(HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "request_id": request_id,
                    "action": action,
                    "stdout": stdout,
                    "stderr": stderr,
                })))
            }
        }
        Err(err) => Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "request_id": request_id,
            "action": action,
            "error": format!("Failed to execute systemctl: {}", err),
        }))),
    }
}

// ============================================================================
// DOCKER ACTIONS
// ============================================================================

/// Docker action request
#[derive(Debug, serde::Deserialize)]
pub(crate) struct DockerActionRequest {
    action: String,
    container: Option<String>,
}

/// POST /monitoring/docker/action
/// Execute docker compose actions (restart, stop, start, logs)
pub(crate) async fn docker_action(
    body: web::Json<DockerActionRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let action = &body.action;
    let container = body.container.as_deref();

    info!(
        "Docker action requested: {} container={:?}",
        action, container
    );

    let (cmd, args): (&str, Vec<&str>) = match action.as_str() {
        "restart" => {
            if let Some(c) = container {
                ("docker", vec!["restart", c])
            } else {
                (
                    "docker",
                    vec!["compose", "-f", "docker-compose.yml", "restart"],
                )
            }
        }
        "stop" => {
            if let Some(c) = container {
                ("docker", vec!["stop", c])
            } else {
                (
                    "docker",
                    vec!["compose", "-f", "docker-compose.yml", "stop"],
                )
            }
        }
        "start" => {
            if let Some(c) = container {
                ("docker", vec!["start", c])
            } else {
                (
                    "docker",
                    vec!["compose", "-f", "docker-compose.yml", "up", "-d"],
                )
            }
        }
        "down" => (
            "docker",
            vec!["compose", "-f", "docker-compose.yml", "down"],
        ),
        "up" => (
            "docker",
            vec!["compose", "-f", "docker-compose.yml", "up", "-d"],
        ),
        _ => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Unknown action: {}", action)
            })));
        }
    };

    // Execute the command
    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .current_dir("/home/pde/ag")
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();

            if success {
                info!("Docker action {} completed successfully", action);
                Ok(HttpResponse::Ok().json(json!({
                    "status": "ok",
                    "request_id": request_id,
                    "action": action,
                    "success": true,
                    "stdout": stdout,
                    "stderr": stderr
                })))
            } else {
                warn!("Docker action {} failed: {}", action, stderr);
                Ok(HttpResponse::Ok().json(json!({
                    "status": "error",
                    "request_id": request_id,
                    "action": action,
                    "success": false,
                    "stdout": stdout,
                    "stderr": stderr
                })))
            }
        }
        Err(e) => {
            error!("Failed to execute docker action {}: {}", action, e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Failed to execute: {}", e)
            })))
        }
    }
}
