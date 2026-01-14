// src/api/tool_routes.rs
// Phase 9: Tool Integration API Routes
// Exposes tool selection and execution as HTTP endpoints

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use tracing::info;

use crate::tools::tool_selector::ToolSelector;

// ============ Request/Response Types ============

#[derive(Debug, Deserialize)]
pub struct ToolQueryRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct ToolSelectionResponse {
    pub intent: String,
    pub primary_tool: String,
    pub secondary_tools: Vec<String>,
    pub confidence: f32,
    pub reasoning: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ToolExecutionResponse {
    pub query: String,
    pub selected_tool: String,
    pub intent: String,
    pub confidence: f32,
    pub execution_plan: Vec<ExecutionStep>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ExecutionStep {
    pub step: usize,
    pub tool: String,
    pub action: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AvailableToolsResponse {
    pub tools: Vec<ToolInfo>,
    pub total_tools: usize,
}

#[derive(Debug, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub success_rate: f32,
}

#[derive(Debug, Serialize)]
pub struct ToolSuggestionResponse {
    pub intent: String,
    pub suggestions: Vec<String>,
    pub timestamp: String,
}

// ============ Tool Route Handlers ============

/// Analyze query and suggest best tool
pub async fn analyze_tools(
    req: web::Json<ToolQueryRequest>,
) -> ActixResult<HttpResponse> {
    info!(query = %req.query, "Analyzing tools for query");

    let selection = ToolSelector::select_tools(&req.query);

    let response = ToolSelectionResponse {
        intent: selection.intent.to_string(),
        primary_tool: selection.primary_tool.to_string(),
        secondary_tools: selection.secondary_tools.iter().map(|t| t.to_string()).collect(),
        confidence: selection.confidence,
        reasoning: selection.reasoning,
        timestamp: Utc::now().to_rfc3339(),
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Execute query with automatic tool selection
pub async fn execute_with_tools(
    req: web::Json<ToolQueryRequest>,
) -> ActixResult<HttpResponse> {
    info!(query = %req.query, "Executing query with tools");

    let selection = ToolSelector::select_tools(&req.query);

    // Build execution plan
    let mut execution_plan = vec![
        ExecutionStep {
            step: 1,
            tool: "ToolSelector".to_string(),
            action: "Analyze query intent".to_string(),
            status: "completed".to_string(),
        },
        ExecutionStep {
            step: 2,
            tool: selection.primary_tool.to_string(),
            action: format!("Execute {}", selection.primary_tool.to_string()),
            status: "queued".to_string(),
        },
    ];

    // Add fallback steps
    if !selection.secondary_tools.is_empty() {
        execution_plan.push(ExecutionStep {
            step: 3,
            tool: selection.secondary_tools[0].to_string(),
            action: format!("Fallback to {}", selection.secondary_tools[0].to_string()),
            status: "standby".to_string(),
        });
    }

    let response = ToolExecutionResponse {
        query: req.query.clone(),
        selected_tool: selection.primary_tool.to_string(),
        intent: selection.intent.to_string(),
        confidence: selection.confidence,
        execution_plan,
        timestamp: Utc::now().to_rfc3339(),
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Get list of available tools
pub async fn list_available_tools() -> ActixResult<HttpResponse> {
    info!("Listing available tools");

    let tools = vec![
        ToolInfo {
            name: "Calculator".to_string(),
            description: "Perform mathematical calculations and arithmetic operations".to_string(),
            success_rate: 0.95,
        },
        ToolInfo {
            name: "WebSearch".to_string(),
            description: "Search the web for recent information".to_string(),
            success_rate: 0.85,
        },
        ToolInfo {
            name: "URLFetch".to_string(),
            description: "Fetch and extract content from URLs".to_string(),
            success_rate: 0.80,
        },
        ToolInfo {
            name: "SemanticSearch".to_string(),
            description: "Search the local knowledge base semantically".to_string(),
            success_rate: 0.75,
        },
        ToolInfo {
            name: "DatabaseQuery".to_string(),
            description: "Query structured data from databases".to_string(),
            success_rate: 0.90,
        },
        ToolInfo {
            name: "Translator".to_string(),
            description: "Translate text between supported languages offline".to_string(),
            success_rate: 0.88,
        },
        ToolInfo {
            name: "SentimentAnalyzer".to_string(),
            description: "Classify the sentiment of text with confidence scores".to_string(),
            success_rate: 0.90,
        },
        ToolInfo {
            name: "EntityExtractor".to_string(),
            description: "Extract people, places, organizations, and other entities".to_string(),
            success_rate: 0.87,
        },
        ToolInfo {
            name: "SpellChecker".to_string(),
            description: "Detect and correct spelling mistakes with suggestions".to_string(),
            success_rate: 0.96,
        },
        ToolInfo {
            name: "Scheduler".to_string(),
            description: "Create lightweight reminders like 'schedule sync in 30 minutes'".to_string(),
            success_rate: 0.90,
        },
        ToolInfo {
            name: "Memory".to_string(),
            description: "Store, search, and forget agent memories in the local DB".to_string(),
            success_rate: 0.88,
        },
    ];

    let response = AvailableToolsResponse {
        total_tools: tools.len(),
        tools,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Detect query intent
pub async fn detect_intent(
    req: web::Json<ToolQueryRequest>,
) -> ActixResult<HttpResponse> {
    info!(query = %req.query, "Detecting query intent");

    let intent = ToolSelector::detect_intent(&req.query);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "query": req.query,
        "intent": intent.to_string(),
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// Suggest tools without executing them
pub async fn suggest_tools(
    req: web::Json<ToolQueryRequest>,
) -> ActixResult<HttpResponse> {
    let selection = ToolSelector::select_tools(&req.query);
    let mut suggestions = vec![selection.primary_tool.to_string()];
    suggestions.extend(selection.secondary_tools.iter().map(|t| t.to_string()));

    Ok(HttpResponse::Ok().json(ToolSuggestionResponse {
        intent: selection.intent.to_string(),
        suggestions,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

// ============ Route Configuration ============

pub fn configure_tool_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/tools")
            .route("/analyze", web::post().to(analyze_tools))
            .route("/execute", web::post().to(execute_with_tools))
            .route("/available", web::get().to(list_available_tools))
            .route("/detect-intent", web::post().to(detect_intent))
            .route("/suggest", web::post().to(suggest_tools))
    );
}
