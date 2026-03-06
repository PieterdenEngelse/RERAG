use crate::api::{self, RagMemoryItem};
use crate::app::{ClearChat, ShowRagInfo};
use crate::components::BackendSelector;
use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Request, RequestInit, RequestMode, Response};

fn backend_origin() -> String {
    if let Some(window) = web_sys::window() {
        let location = window.location();
        if let Ok(origin) = location.origin() {
            let is_loopback = origin.contains("127.0.0.1") || origin.contains("localhost");
            if !is_loopback {
                return origin;
            }

            let hostname = location
                .hostname()
                .unwrap_or_else(|_| "127.0.0.1".into())
                .trim()
                .to_string();
            let scheme = location
                .protocol()
                .unwrap_or_else(|_| "http:".into())
                .trim_end_matches(':')
                .to_string();

            if hostname.is_empty() {
                return "http://127.0.0.1:3010".to_string();
            }

            return format!("{}://{}:3010", scheme, hostname);
        }
    }

    "http://127.0.0.1:3010".to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub context: Option<String>, // RAG context used (if any)
}

fn backend_agent_url() -> String {
    format!("{}/agent", backend_origin())
}

fn backend_stream_url() -> String {
    format!("{}/agent/stream", backend_origin())
}

/// Check if input is a chat command (starts with /)
fn is_chat_command(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("/help")
        || trimmed.starts_with("/goal")
        || trimmed.starts_with("/goals")
        || trimmed.starts_with("/status")
        || trimmed.starts_with("/models")
        || trimmed.starts_with("/clear")
        || trimmed.starts_with("/focus")
        || trimmed.starts_with("/unfocus")
        || trimmed.starts_with("/persona")
        || trimmed.starts_with("/verbose")
        || trimmed.starts_with("/brief")
        || trimmed.starts_with("/run")
        || trimmed.starts_with("/chain")
        || trimmed.starts_with("/retry")
        || trimmed.starts_with("/undo")
        || trimmed.starts_with("/dry-run")
        || trimmed.starts_with("/model")
        || trimmed.starts_with("/temperature")
        || trimmed.starts_with("/export")
        || trimmed.starts_with("/import")
        || trimmed.starts_with("/debug")
        || trimmed.starts_with("/tokens")
        || trimmed.starts_with("/forget")
        || trimmed.starts_with("/history")
        || trimmed.starts_with("/sources")
        || trimmed.starts_with("/learn")
        || trimmed.starts_with("/note")
        || trimmed.starts_with("/subgoal")
        || trimmed.starts_with("/pause")
        || trimmed.starts_with("/resume")
        || trimmed.starts_with("/abandon")
        || trimmed.starts_with("/reflect")
        || trimmed.starts_with("/why")
}

/// Convert model names to friendly display names
fn friendly_model_name(name: &str) -> String {
    match name {
        "phi:latest" => "Phi-2".to_string(),
        "phi3.5:latest" => "Phi-3.5".to_string(),
        "phi3:latest" => "Phi-3".to_string(),
        "llama3.2:latest" | "llama3.2:3b" => "Llama 3.2 (3B)".to_string(),
        "llama3.2:1b" => "Llama 3.2 (1B)".to_string(),
        "llama3:latest" | "llama3:8b" => "Llama 3 (8B)".to_string(),
        "mistral:latest" | "mistral:7b" => "Mistral (7B)".to_string(),
        "gemma2:latest" | "gemma2:9b" => "Gemma 2 (9B)".to_string(),
        "gemma2:2b" => "Gemma 2 (2B)".to_string(),
        "qwen2.5:latest" | "qwen2.5:7b" => "Qwen 2.5 (7B)".to_string(),
        "qwen2.5:3b" => "Qwen 2.5 (3B)".to_string(),
        "nomic-embed-text:latest" => "Nomic Embed".to_string(),
        _ => {
            // For unknown models, clean up the name a bit
            name.replace(":latest", "").replace(":", " ")
        }
    }
}

#[derive(Deserialize)]
struct AgentCommandResponse {
    response: AgentResponseInner,
    #[allow(dead_code)]
    request_id: String,
}

#[derive(Deserialize)]
struct AgentResponseInner {
    answer: String,
    #[serde(default)]
    #[allow(dead_code)]
    chunks_used: usize,
}

#[component]
pub fn Home() -> Element {
    let mut messages = use_signal(|| Vec::<ChatMessage>::new());
    let mut input_text = use_signal(|| String::new());
    let mut is_loading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut selected_model = use_signal(|| "phi:latest".to_string());
    let mut cancel_requested = use_signal(|| false);

    // Available models for dropdown
    let available_models: Signal<Vec<api::ModelInfo>> = use_signal(Vec::new);
    let models_loading = use_signal(|| false);

    // File upload state
    let mut show_upload_panel = use_signal(|| false);
    let mut documents = use_signal(|| Vec::<String>::new());
    let mut upload_status = use_signal(|| Option::<String>::None);
    let is_uploading = use_signal(|| false);
    let mut show_file_types_info = use_signal(|| false);
    let mut show_delete_docs_modal = use_signal(|| false);
    let mut selected_documents = use_signal(|| Vec::<String>::new());
    let mut deleting_documents = use_signal(|| false);
    let mut delete_docs_status = use_signal(|| Option::<String>::None);

    // Chat mode: "rag", "llm", or "hybrid"
    let mut chat_mode = use_signal(|| "hybrid".to_string());

    let mut show_delete_memories_modal = use_signal(|| false);
    let mut rag_memories = use_signal(|| Vec::<RagMemoryItem>::new());
    let mut memories_loading = use_signal(|| false);
    let mut selected_memories = use_signal(|| Vec::<i64>::new());
    let mut deleting_memories = use_signal(|| false);
    let mut delete_memories_status = use_signal(|| Option::<String>::None);
    let mut memory_error = use_signal(|| Option::<String>::None);

    // Info panel state (global context)
    let mut show_info = use_context::<Signal<ShowRagInfo>>();
    let mut runtime_suspended = use_context::<Signal<crate::app::RuntimeSuspended>>();

    // Clear chat signal (triggered by Home link in header)
    let clear_chat = use_context::<Signal<ClearChat>>();

    // Watch for clear chat signal
    use_effect(move || {
        if clear_chat().0 {
            messages.write().clear();
            input_text.set(String::new());
            error_msg.set(None);

            let mut clear_chat = clear_chat.clone();
            spawn(async move {
                gloo_timers::future::TimeoutFuture::new(0).await;
                clear_chat.set(ClearChat(false));
            });
        }
    });

    // Help modal state
    let mut show_help_modal = use_signal(|| false);
    let mut help_content = use_signal(|| String::new());

    // Mode info modal states
    let mut show_rag_info = use_signal(|| false);
    let mut show_llm_info = use_signal(|| false);
    let mut show_hybrid_info = use_signal(|| false);

    // Training feedback state - track last response for rating
    let mut last_query = use_signal(|| String::new());
    let mut last_response = use_signal(|| String::new());
    let mut last_context = use_signal(|| Option::<String>::None);
    let mut last_response_rated = use_signal(|| false);
    let mut feedback_status = use_signal(|| Option::<String>::None);

    // Prompt caching toggle (uses /api/chat instead of /api/generate for KV cache reuse)
    let mut prompt_caching_enabled = use_signal(|| false);
    let mut show_cache_info = use_signal(|| false);
    let mut show_api_behavior = use_signal(|| false);
    let mut show_kv_details = use_signal(|| false);
    let mut show_attention_details = use_signal(|| false);

    // Backend type state for home page board
    let mut current_backend = use_signal(|| String::from("ollama"));
    let mut show_backend_info = use_signal(|| false);

    // Load documents on mount
    use_effect(move || {
        spawn(async move {
            match api::list_documents().await {
                Ok(mut resp) => {
                    resp.documents.sort();
                    documents.set(resp.documents);
                }
                Err(_) => {} // Silently fail
            }
        });
    });

    // Load prompt caching state on mount
    {
        let mut prompt_caching_enabled = prompt_caching_enabled.clone();
        use_effect(move || {
            spawn(async move {
                if let Ok(resp) = api::get_prompt_caching().await {
                    prompt_caching_enabled.set(resp.enabled);
                }
            });
        });
    };

    // Load active model and available models from hardware config once on mount
    {
        let mut selected_model = selected_model.clone();
        let mut available_models = available_models.clone();
        let mut models_loading = models_loading.clone();
        let mut error_signal = error_msg.clone();
        let mut current_backend = current_backend.clone();
        use_future(move || async move {
            // Try to load hardware config (with a quick retry) to keep home page in sync
            let mut last_error = None;
            let mut backend_type = String::new();
            let origin = backend_origin();

            for attempt in 0..2 {
                let fetch_result = match api::fetch_hardware_config_with_origin(&origin).await {
                    Ok(resp) => Ok(resp),
                    Err(primary_err) => match api::fetch_hardware_config().await {
                        Ok(resp) => Ok(resp),
                        Err(fallback_err) => {
                            last_error = Some(format!(
                                "primary: {}; fallback: {}",
                                primary_err, fallback_err
                            ));
                            Err(())
                        }
                    },
                };

                match fetch_result {
                    Ok(resp) => {
                        let active_model = resp.config.model.trim().to_string();
                        if !active_model.is_empty() {
                            selected_model.set(active_model);
                        }
                        backend_type = resp.config.backend_type.clone();
                        current_backend.set(backend_type.clone());
                        last_error = None;
                        break;
                    }
                    Err(()) => {
                        if attempt == 0 {
                            gloo_timers::future::TimeoutFuture::new(250).await;
                        }
                    }
                }
            }

            if let Some(err) = last_error {
                error_signal.set(Some(format!(
                    "[INFO] Failed to load active model from hardware config: {}",
                    err
                )));
                return;
            }

            // Load available models for the backend
            if !backend_type.is_empty() {
                models_loading.set(true);
                match api::fetch_models(&backend_type).await {
                    Ok(models) => {
                        available_models.set(models);
                    }
                    Err(_) => {
                        // Silently fail - models dropdown will just be empty
                    }
                }
                models_loading.set(false);
            }
        });
    };

    let send_message = move |_evt: Event<MouseData>| {
        let user_input = input_text().trim().to_string();
        if user_input.is_empty() || is_loading() {
            return;
        }

        // Handle /clear command locally
        if user_input.trim() == "/clear" {
            messages.write().clear();
            input_text.set(String::new());
            return;
        }

        messages.write().push(ChatMessage {
            role: "user".to_string(),
            content: user_input.clone(),
            context: None,
        });

        input_text.set(String::new());
        is_loading.set(true);
        error_msg.set(None);
        cancel_requested.set(false);

        let cancel_flag = cancel_requested.clone();
        let mode = chat_mode();

        spawn(async move {
            // Check if this is a chat command - route to backend
            if is_chat_command(&user_input) {
                let body = serde_json::json!({ "query": user_input, "mode": mode });
                let request = gloo_net::http::Request::post(&backend_agent_url())
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .unwrap();
                match request.send().await {
                    Ok(response) => {
                        if cancel_flag() {
                            is_loading.set(false);
                            return;
                        }
                        if response.ok() {
                            match response.json::<AgentCommandResponse>().await {
                                Ok(data) => {
                                    if !cancel_flag() {
                                        // Check if this is a help response - show in modal
                                        if user_input.trim() == "/help"
                                            || data.response.answer.contains("Available Commands")
                                        {
                                            help_content.set(data.response.answer);
                                            show_help_modal.set(true);
                                        } else {
                                            messages.write().push(ChatMessage {
                                                role: "assistant".to_string(),
                                                content: data.response.answer,
                                                context: None,
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    error_msg.set(Some(format!(
                                        "Failed to parse command response: {}",
                                        e
                                    )));
                                }
                            }
                        } else {
                            error_msg
                                .set(Some(format!("Command failed: HTTP {}", response.status())));
                        }
                    }
                    Err(e) => {
                        error_msg.set(Some(format!("Command request failed: {}", e)));
                    }
                }
                is_loading.set(false);
                return;
            }

            // Regular message - use streaming for LLM/Hybrid modes
            // Add an empty assistant message that we'll update with streamed content
            messages.write().push(ChatMessage {
                role: "assistant".to_string(),
                content: String::new(),
                context: None,
            });
            let msg_index = messages().len() - 1;

            // Use streaming endpoint
            let body = serde_json::json!({ "query": user_input, "mode": mode });

            // Create fetch request with streaming
            let window = web_sys::window().unwrap();
            let opts = RequestInit::new();
            opts.set_method("POST");
            opts.set_mode(RequestMode::Cors);
            let body_value = JsValue::from_str(&body.to_string());
            opts.set_body(&body_value);

            let request = Request::new_with_str_and_init(&backend_stream_url(), &opts).unwrap();
            request
                .headers()
                .set("Content-Type", "application/json")
                .unwrap();

            let resp_promise = window.fetch_with_request(&request);

            match wasm_bindgen_futures::JsFuture::from(resp_promise).await {
                Ok(resp_value) => {
                    let response: Response = resp_value.dyn_into().unwrap();

                    if !response.ok() {
                        error_msg.set(Some(format!("HTTP error: {}", response.status())));
                        // Remove the empty message
                        messages.write().pop();
                        is_loading.set(false);
                        return;
                    }

                    // Get the response body as a ReadableStream
                    if let Some(body) = response.body() {
                        let reader = body
                            .get_reader()
                            .dyn_into::<web_sys::ReadableStreamDefaultReader>()
                            .unwrap();
                        let mut accumulated_text = String::new();
                        let mut chunks_used = 0usize;

                        loop {
                            if cancel_flag() {
                                break;
                            }

                            let read_promise = reader.read();
                            match wasm_bindgen_futures::JsFuture::from(read_promise).await {
                                Ok(result) => {
                                    let done =
                                        js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                                            .unwrap()
                                            .as_bool()
                                            .unwrap_or(true);

                                    if done {
                                        break;
                                    }

                                    let value =
                                        js_sys::Reflect::get(&result, &JsValue::from_str("value"))
                                            .unwrap();
                                    let array = js_sys::Uint8Array::new(&value);
                                    let bytes = array.to_vec();
                                    let text = String::from_utf8_lossy(&bytes);

                                    // Parse SSE events
                                    for line in text.lines() {
                                        if line.starts_with("data: ") {
                                            let json_str = &line[6..];
                                            if let Ok(event) =
                                                serde_json::from_str::<serde_json::Value>(json_str)
                                            {
                                                if let Some(event_type) =
                                                    event.get("type").and_then(|v| v.as_str())
                                                {
                                                    match event_type {
                                                        "token" => {
                                                            if let Some(content) = event
                                                                .get("content")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                accumulated_text.push_str(content);
                                                                // Update the message in place
                                                                if let Some(msg) = messages
                                                                    .write()
                                                                    .get_mut(msg_index)
                                                                {
                                                                    msg.content =
                                                                        accumulated_text.clone();
                                                                }
                                                            }
                                                        }
                                                        "done" => {
                                                            chunks_used = event
                                                                .get("chunks_used")
                                                                .and_then(|v| v.as_u64())
                                                                .unwrap_or(0)
                                                                as usize;
                                                        }
                                                        "complete" => {
                                                            // Non-streaming response (for RAG mode)
                                                            if let Some(answer) = event
                                                                .get("answer")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                accumulated_text =
                                                                    answer.to_string();
                                                                if let Some(msg) = messages
                                                                    .write()
                                                                    .get_mut(msg_index)
                                                                {
                                                                    msg.content =
                                                                        accumulated_text.clone();
                                                                }
                                                            }
                                                        }
                                                        "error" => {
                                                            if let Some(err_msg) = event
                                                                .get("message")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                error_msg
                                                                    .set(Some(err_msg.to_string()));
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }

                        // Update context if chunks were used
                        let ctx = if chunks_used > 0 {
                            let ctx_str =
                                format!("Used {} chunks from knowledge base", chunks_used);
                            if let Some(msg) = messages.write().get_mut(msg_index) {
                                msg.context = Some(ctx_str.clone());
                            }
                            Some(ctx_str)
                        } else {
                            None
                        };

                        // Track for feedback buttons
                        last_query.set(user_input.clone());
                        last_response.set(accumulated_text);
                        last_context.set(ctx);
                        last_response_rated.set(false);
                    }
                }
                Err(e) => {
                    error_msg.set(Some(format!(
                        "Request failed: {:?}. Is backend running?",
                        e
                    )));
                    // Remove the empty message
                    messages.write().pop();
                }
            }

            is_loading.set(false);
        });
    };

    let on_keypress = move |evt: Event<KeyboardData>| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            let user_input = input_text().trim().to_string();
            if user_input.is_empty() || is_loading() {
                return;
            }

            if user_input.trim() == "/clear" {
                messages.write().clear();
                input_text.set(String::new());
                return;
            }

            messages.write().push(ChatMessage {
                role: "user".to_string(),
                content: user_input.clone(),
                context: None,
            });

            input_text.set(String::new());
            is_loading.set(true);
            error_msg.set(None);
            cancel_requested.set(false);

            let cancel_flag = cancel_requested.clone();
            let mode = chat_mode();

            spawn(async move {
                if is_chat_command(&user_input) {
                    let body = serde_json::json!({ "query": user_input, "mode": mode });
                    let request = gloo_net::http::Request::post(&backend_agent_url())
                        .header("Content-Type", "application/json")
                        .body(body.to_string())
                        .unwrap();
                    match request.send().await {
                        Ok(response) => {
                            if cancel_flag() {
                                is_loading.set(false);
                                return;
                            }
                            if response.ok() {
                                match response.json::<AgentCommandResponse>().await {
                                    Ok(data) => {
                                        if !cancel_flag() {
                                            if user_input.trim() == "/help"
                                                || data
                                                    .response
                                                    .answer
                                                    .contains("Available Commands")
                                            {
                                                help_content.set(data.response.answer);
                                                show_help_modal.set(true);
                                            } else {
                                                messages.write().push(ChatMessage {
                                                    role: "assistant".to_string(),
                                                    content: data.response.answer,
                                                    context: None,
                                                });
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error_msg.set(Some(format!(
                                            "Failed to parse command response: {}",
                                            e
                                        )));
                                    }
                                }
                            } else {
                                error_msg.set(Some(format!(
                                    "Command failed: HTTP {}",
                                    response.status()
                                )));
                            }
                        }
                        Err(e) => {
                            error_msg.set(Some(format!("Command request failed: {}", e)));
                        }
                    }
                    is_loading.set(false);
                    return;
                }

                messages.write().push(ChatMessage {
                    role: "assistant".to_string(),
                    content: String::new(),
                    context: None,
                });
                let msg_index = messages().len() - 1;

                let body = serde_json::json!({ "query": user_input, "mode": mode });

                let window = web_sys::window().unwrap();
                let opts = RequestInit::new();
                opts.set_method("POST");
                opts.set_mode(RequestMode::Cors);
                let body_value = JsValue::from_str(&body.to_string());
                opts.set_body(&body_value);

                let request = Request::new_with_str_and_init(&backend_stream_url(), &opts).unwrap();
                request
                    .headers()
                    .set("Content-Type", "application/json")
                    .unwrap();

                let resp_promise = window.fetch_with_request(&request);

                match wasm_bindgen_futures::JsFuture::from(resp_promise).await {
                    Ok(resp_value) => {
                        let response: Response = resp_value.dyn_into().unwrap();

                        if !response.ok() {
                            error_msg.set(Some(format!("HTTP error: {}", response.status())));
                            messages.write().pop();
                            is_loading.set(false);
                            return;
                        }

                        if let Some(body) = response.body() {
                            let reader = body
                                .get_reader()
                                .dyn_into::<web_sys::ReadableStreamDefaultReader>()
                                .unwrap();
                            let mut accumulated_text = String::new();
                            let mut chunks_used = 0usize;

                            loop {
                                if cancel_flag() {
                                    break;
                                }

                                let read_promise = reader.read();
                                match wasm_bindgen_futures::JsFuture::from(read_promise).await {
                                    Ok(result) => {
                                        let done = js_sys::Reflect::get(
                                            &result,
                                            &JsValue::from_str("done"),
                                        )
                                        .unwrap()
                                        .as_bool()
                                        .unwrap_or(true);

                                        if done {
                                            break;
                                        }

                                        let value = js_sys::Reflect::get(
                                            &result,
                                            &JsValue::from_str("value"),
                                        )
                                        .unwrap();
                                        let array = js_sys::Uint8Array::new(&value);
                                        let bytes = array.to_vec();
                                        let text = String::from_utf8_lossy(&bytes);

                                        for line in text.lines() {
                                            if line.starts_with("data: ") {
                                                let json_str = &line[6..];
                                                if let Ok(event) =
                                                    serde_json::from_str::<serde_json::Value>(
                                                        json_str,
                                                    )
                                                {
                                                    if let Some(event_type) =
                                                        event.get("type").and_then(|v| v.as_str())
                                                    {
                                                        match event_type {
                                                            "token" => {
                                                                if let Some(content) = event
                                                                    .get("content")
                                                                    .and_then(|v| v.as_str())
                                                                {
                                                                    accumulated_text
                                                                        .push_str(content);
                                                                    if let Some(msg) = messages
                                                                        .write()
                                                                        .get_mut(msg_index)
                                                                    {
                                                                        msg.content =
                                                                            accumulated_text
                                                                                .clone();
                                                                    }
                                                                }
                                                            }
                                                            "done" => {
                                                                chunks_used = event
                                                                    .get("chunks_used")
                                                                    .and_then(|v| v.as_u64())
                                                                    .unwrap_or(0)
                                                                    as usize;
                                                            }
                                                            "complete" => {
                                                                if let Some(answer) = event
                                                                    .get("answer")
                                                                    .and_then(|v| v.as_str())
                                                                {
                                                                    accumulated_text =
                                                                        answer.to_string();
                                                                    if let Some(msg) = messages
                                                                        .write()
                                                                        .get_mut(msg_index)
                                                                    {
                                                                        msg.content =
                                                                            accumulated_text
                                                                                .clone();
                                                                    }
                                                                }
                                                            }
                                                            "error" => {
                                                                if let Some(err_msg) = event
                                                                    .get("message")
                                                                    .and_then(|v| v.as_str())
                                                                {
                                                                    error_msg.set(Some(
                                                                        err_msg.to_string(),
                                                                    ));
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }

                            if chunks_used > 0 {
                                if let Some(msg) = messages.write().get_mut(msg_index) {
                                    msg.context = Some(format!(
                                        "Used {} chunks from knowledge base",
                                        chunks_used
                                    ));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error_msg.set(Some(format!(
                            "Request failed: {:?}. Is backend running?",
                            e
                        )));
                        messages.write().pop();
                    }
                }

                is_loading.set(false);
            });
        }
    };

    let _clear_chat_local = move |_evt: Event<MouseData>| {
        messages.write().clear();
        error_msg.set(None);
    };

    let toggle_upload_panel = move |_evt: Event<MouseData>| {
        show_upload_panel.set(!show_upload_panel());
    };

    let refresh_documents = move |_evt: Event<MouseData>| {
        spawn(async move {
            match api::list_documents().await {
                Ok(resp) => documents.set(resp.documents),
                Err(e) => upload_status.set(Some(format!("Failed to load: {}", e))),
            }
        });
    };

    let mut toggle_doc_selection = {
        let mut selected_documents = selected_documents.clone();
        move |name: String| {
            let mut current = selected_documents.write();
            if let Some(idx) = current.iter().position(|d| d == &name) {
                current.remove(idx);
            } else {
                current.push(name);
            }
        }
    };

    let mut toggle_memory_selection = {
        let mut selected_memories = selected_memories.clone();
        move |id: i64| {
            let mut current = selected_memories.write();
            if let Some(idx) = current.iter().position(|m| *m == id) {
                current.remove(idx);
            } else {
                current.push(id);
            }
        }
    };

    let stop_request = move |_evt: Event<MouseData>| {
        cancel_requested.set(true);
        is_loading.set(false);
        error_msg.set(Some("[INFO] Request cancelled.".to_string()));
    };

    rsx! {
        // Welcome text with RAG toggle - fixed centered relative to full viewport (aligns with header title)
        if messages().is_empty() {
            div {
                class: "fixed inset-x-0 z-10 pointer-events-none",
                style: "top: 3rem;",
                div {
                    class: "max-w-2xl mx-auto w-full flex flex-col items-center",
                    style: "padding-left: 0.5cm; padding-right: 0.5cm; width: calc(min(90vw, 34rem));",
                    div {
                        class: "flex flex-col items-center w-full gap-4 pointer-events-auto",
                        style: "transform: translateY(1cm);",
                        p { class: "text-xs text-base-content/60 text-center", "Use these to set the starting context" }

                        // Row with Backend, Mode and RAG boards side by side
                        div {
                            class: "flex justify-center gap-4 w-full",

                            // Backend board
                            div {
                                class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                                style: "min-width: 12rem;",
                                div {
                                    class: "flex items-center gap-2",
                                    label {
                                        class: "font-medium text-center",
                                        style: "color: white; font-size: 1.1rem;",
                                        "Runtime"
                                    }
                                    button {
                                        class: "shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80 pointer-events-auto",
                                        style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                        onclick: move |evt| {
                                            evt.stop_propagation();
                                            show_backend_info.set(true);
                                        },
                                        title: "Info about runtime selection",
                                        svg {
                                            class: INFO_ICON_SVG_CLASS,
                                            view_box: "0 0 20 20",
                                            fill: "none",
                                            stroke: "#026B7C",
                                            stroke_width: "1.5",
                                            circle { cx: "10", cy: "10", r: "9" }
                                            line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                            circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                        }
                                    }
                                }
                                BackendSelector {
                                    current_backend: current_backend,
                                    clear_model_on_change: true,
                                    show_save_button: true,
                                    show_info_button: false,
                                }
                            }

                            // Mode board
                            div {
                                class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                                label {
                                    class: "font-medium text-center",
                                    style: "color: white; font-size: 1.1rem;",
                                    "Mode"
                                }
                                div {
                                    class: "flex justify-center",
                                    div {
                                        class: "flex",
                                        style: "gap: 1.08rem;",
                                    // RAG mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "rag" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| chat_mode.set("rag".to_string()),
                                            title: "Search documents only",
                                            span { style: "font-size: 0.75em;", "📚" }
                                            " RAG"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_rag_info.set(true),
                                            title: "Info about RAG mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                    // LLM mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "llm" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| chat_mode.set("llm".to_string()),
                                            title: "Use LLM only (no document search)",
                                            span { style: "font-size: 0.75em;", "🤖" }
                                            " LLM"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_llm_info.set(true),
                                            title: "Info about LLM mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                    // Hybrid mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "hybrid" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| chat_mode.set("hybrid".to_string()),
                                            title: "Search documents + LLM enhancement",
                                            span { style: "font-size: 0.75em;", "⚡" }
                                            " Hybrid"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_hybrid_info.set(true),
                                            title: "Info about Hybrid mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                }
                            }
                            p {
                                class: "text-xs font-medium text-center",
                                style: "color: white;",
                                match chat_mode().as_str() {
                                    "rag" => "RAG mode - searches your documents only",
                                    "llm" => "LLM mode - uses AI without document search",
                                    "hybrid" => "Hybrid mode - documents + AI fallback",
                                    _ => "Select a mode"
                                }
                            }
                            }

                                                    // RAG board
                            div {
                                class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                                style: "min-width: calc(12rem + 2cm); padding-left: calc(1.25rem + 1cm); padding-right: calc(1.25rem + 1cm);",
                                label {
                                    class: "font-medium text-center",
                                    style: "color: white; font-size: 1.1rem;",
                                    "RAG Add's"
                                }
                                div {
                                    class: "flex justify-center",
                                    style: "gap: 1.08rem;",
                                    // Documents buttons
                                    div {
                                        class: "flex flex-col items-center",
                                        style: "width: 7.5rem;",
                                        div { class: "flex gap-1",
                                            // Info (standard styling)
                                            button {
                                                class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                                style: "width: 2rem; height: 2rem; min-width: 2rem; min-height: 2rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                                onclick: move |evt| {
                                                    evt.stop_propagation();
                                                    show_info.set(ShowRagInfo(true));
                                                },
                                                title: "Info about documents",
                                                svg {
                                                    class: INFO_ICON_SVG_CLASS,
                                                    view_box: "0 0 20 20",
                                                    fill: "none",
                                                    stroke: "#026B7C",
                                                    circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                    line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                    circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                                }
                                            }
                                            button {
                                                class: "btn rounded-full px-4 text-xl font-bold",
                                                style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                                onclick: move |_| show_upload_panel.set(!show_upload_panel()),
                                                title: "Toggle documents panel",
                                                "+"
                                            }
                                            button {
                                                class: "btn rounded-full px-4 text-xl font-bold text-white",
                                                style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                                onclick: move |_| {
                                                    show_delete_docs_modal.set(true);
                                                    spawn(async move {
                                                        match api::list_documents().await {
                                                            Ok(mut resp) => {
                                                                resp.documents.sort();
                                                                documents.set(resp.documents);
                                                            }
                                                            Err(e) => upload_status.set(Some(format!("Failed to load: {}", e))),
                                                        }
                                                    });
                                                },
                                                title: "Delete documents",
                                                "-"
                                            }
                                        }
                                        span {
                                            class: "text-sm mt-1 font-medium",
                                            style: "color: white;",
                                            "Documents"
                                        }
                                    }
                                    // Memories buttons
                                    div {
                                        class: "flex flex-col items-center",
                                        style: "width: 7.5rem;",
                                        div { class: "flex gap-1",
                                            a {
                                                class: "btn rounded-full px-4 text-xl font-bold cursor-pointer",
                                                style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none; text-decoration: none;",
                                                href: "/config/memories",
                                                title: "Add RAG memories",
                                                "+"
                                            }
                                            button {
                                                class: "btn rounded-full px-4 text-xl font-bold text-white",
                                                style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                                onclick: move |_| {
                                                    show_delete_memories_modal.set(true);
                                                    memories_loading.set(true);
                                                    memory_error.set(None);
                                                    spawn(async move {
                                                        match api::fetch_rag_memories(100).await {
                                                            Ok(resp) => rag_memories.set(resp.memories),
                                                            Err(e) => memory_error.set(Some(e)),
                                                        }
                                                        memories_loading.set(false);
                                                    });
                                                },
                                                title: "Delete memories",
                                                "-"
                                            }
                                            // Info (standard styling)
                                            button {
                                                class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                                style: "width: 2rem; height: 2rem; min-width: 2rem; min-height: 2rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                                onclick: move |evt| {
                                                    evt.stop_propagation();
                                                    show_info.set(ShowRagInfo(true));
                                                },
                                                title: "Info about memories",
                                                svg {
                                                    class: INFO_ICON_SVG_CLASS,
                                                    view_box: "0 0 20 20",
                                                    fill: "none",
                                                    stroke: "#026B7C",
                                                    circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                    line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                    circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                                }
                                            }
                                        }
                                        span {
                                            class: "text-sm mt-1 font-medium",
                                            style: "color: white;",
                                            "Memories"
                                        }
                                    }
                                }
                            }
                        }

                        // KV Cache and Embedding boards - horizontally centered
                        div {
                            class: "flex justify-center gap-4 w-full pointer-events-auto",
                            style: "margin-top: 1cm;",

                            // KV board
                            div {
                                class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-2",
                                label {
                                    class: "font-medium text-center",
                                    style: "color: white; font-size: 1.1rem;",
                                    "KV Cache"
                                }
                                div {
                                class: "flex items-center justify-center gap-6 w-full",
                                div {
                                    class: "flex flex-col items-center gap-1",
                                    div {
                                        class: "flex items-center gap-2",
                                        span {
                                            class: "text-sm font-medium",
                                            style: "color: white;",
                                            "KV Cache"
                                        }
                                        label {
                                            class: "flex items-center gap-2 cursor-pointer pointer-events-auto",
                                            input {
                                                r#type: "checkbox",
                                                class: "toggle toggle-sm !border !border-white",
                                                style: {
                                                    format!(
                                                        "border: 1px solid white; background-color: {};",
                                                        if prompt_caching_enabled() { "" } else { "#d1d5db" }
                                                    )
                                                },
                                                checked: prompt_caching_enabled(),
                                                onchange: move |evt| {
                                                    let new_value = evt.checked();
                                                    prompt_caching_enabled.set(new_value);
                                                    spawn(async move {
                                                        let _ = api::set_prompt_caching(new_value).await;
                                                    });
                                                }
                                            }
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer pointer-events-auto",
                                            style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_cache_info.set(true),
                                            title: "Info about KV caching",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                    p {
                                        class: "text-xs text-center",
                                        style: if prompt_caching_enabled() {
                                            "color: #22c55e;"
                                        } else {
                                            "color: rgba(255,255,255,0.5);"
                                        },
                                        if prompt_caching_enabled() {
                                            "KV Cache enabled"
                                        } else {
                                            "KV Cache disabled"
                                        }
                                    }
                                }
                                } // end inner flex
                            } // end KV board
                        } // end horizontal container
                    }
                }
            }
        }

        // Full height container that fills below the global header
        div {
            class: "relative flex h-[calc(100vh-2.5rem)] bg-base-200 overflow-hidden",
            "data-theme": "dark",

            // Left sidebar - Document Upload Panel (collapsible)
            if show_upload_panel() {
                div {
                    class: "w-64 lg:w-72 bg-base-100 border-r border-base-300 flex flex-col flex-shrink-0 h-full z-20",

                    // Panel header
                    div {
                        class: "p-2 border-b border-base-300 flex justify-between items-center flex-shrink-0",
                        div {
                            class: "flex items-center gap-2",
                            h2 {
                                class: "font-bold text-sm",
                                "📁 Documents"
                            }
                            // Info button for supported file types
                            button {
                                class: "w-5 h-5 min-w-5 min-h-5 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80 pointer-events-auto",
                                style: "background-color: transparent; border: 1.5px solid #026B7C;",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    show_file_types_info.set(true);
                                },
                                svg {
                                    class: INFO_ICON_SVG_CLASS,
                                    view_box: "0 0 20 20",
                                    fill: "none",
                                    stroke: "#026B7C",
                                    circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                    line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                    circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                }
                            }
                        }
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: toggle_upload_panel,
                            "✕"
                        }
                    }

                    // Upload area
                    div {
                        class: "p-2 border-b border-base-300 flex-shrink-0",

                        // File input
                        div {
                            class: "mb-1",
                            p {
                                class: "block text-xs text-base-content/70 mb-1",
                                "Upload documents or code"
                            }
                            // Use label wrapper pattern for reliable file input
                            label {
                                class: "btn btn-xs btn-outline w-full cursor-pointer pointer-events-auto",
                                class: if is_uploading() { "btn-disabled" } else { "" },
                                input {
                                    r#type: "file",
                                    multiple: true,
                                    class: "hidden",
                                    // Documents: pdf, txt, md, html, xml, json
                                    // Code: rs, py, js, ts, go, java, cs, cpp, c, rb, php, sh, sql, yaml, toml
                                    accept: ".pdf,.txt,.text,.md,.markdown,.html,.htm,.xml,.xhtml,.json,.rs,.py,.pyw,.js,.mjs,.cjs,.ts,.tsx,.go,.java,.cs,.cpp,.cc,.cxx,.hpp,.c,.h,.rb,.php,.sh,.bash,.zsh,.sql,.yaml,.yml,.toml",
                                    disabled: is_uploading(),
                                    onchange: {
                                        let is_uploading = is_uploading.clone();
                                        let upload_status = upload_status.clone();
                                        let documents = documents.clone();
                                        move |evt: dioxus::prelude::Event<dioxus::prelude::FormData>| {
                                            let mut is_uploading = is_uploading.clone();
                                            let mut upload_status = upload_status.clone();
                                            let mut documents = documents.clone();
                                            spawn(async move {
                                                is_uploading.set(true);
                                                upload_status.set(Some("Uploading...".to_string()));

                                                // Use Dioxus 0.7 file handling
                                                let files = evt.files();
                                                let total_files = files.len();
                                                let mut success_count = 0;

                                                if total_files > 0 {
                                                    // Heuristic: only stop runtime for "bulk" uploads
                                                    // (3+ files OR any file >=2MB OR total size >=5MB).
                                                    let mut total_bytes: u64 = 0;
                                                    let mut any_large: bool = false;
                                                    for f in &files {
                                                        let size = f.size();
                                                        total_bytes += size;
                                                        if size >= 2 * 1024 * 1024 {
                                                            any_large = true;
                                                        }
                                                    }
                                                    let stop_runtime = total_files >= 3
                                                        || any_large
                                                        || total_bytes >= 5 * 1024 * 1024;

                                                    if stop_runtime {
                                                        upload_status.set(Some(
                                                            "Stopping runtime to free resources for upload…".to_string(),
                                                        ));
                                                        runtime_suspended.set(crate::app::RuntimeSuspended(true));
                                                        let _ = api::runtime_action("stop").await;
                                                    }

                                                    for file_data in &files {
                                                        let file_name = file_data.name();
                                                        upload_status.set(Some(format!("Uploading: {}", file_name)));

                                                        match file_data.read_bytes().await {
                                                            Ok(contents) => {
                                                                match api::upload_document(&file_name, &contents).await {
                                                                    Ok(_resp) => {
                                                                        success_count += 1;
                                                                    }
                                                                    Err(e) => {
                                                                        upload_status.set(Some(format!("✗ {}", e)));
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                upload_status.set(Some(format!("✗ Failed to read: {}", e)));
                                                            }
                                                        }
                                                    }

                                                    if stop_runtime {
                                                        upload_status.set(Some(
                                                            "Starting runtime again…".to_string(),
                                                        ));
                                                        let _ = api::runtime_action("start").await;
                                                        runtime_suspended.set(crate::app::RuntimeSuspended(false));
                                                    }

                                                    // Show final status
                                                    if success_count == total_files {
                                                        upload_status.set(Some(format!("✓ {} file(s) uploaded", success_count)));
                                                    } else if success_count > 0 {
                                                        upload_status.set(Some(format!("⚠ {}/{} uploaded", success_count, total_files)));
                                                    }

                                                    // Refresh document list
                                                    if success_count > 0 {
                                                        if let Ok(mut docs) = api::list_documents().await {
                                                            docs.documents.sort();
                                                            documents.set(docs.documents);
                                                        }
                                                    }
                                                } else {
                                                    upload_status.set(Some("✗ No files selected".to_string()));
                                                }

                                                is_uploading.set(false);

                                                // Clear status after 3 seconds using spawn
                                                let mut upload_status_clear = upload_status.clone();
                                                spawn(async move {
                                                    gloo_timers::future::TimeoutFuture::new(3000).await;
                                                    upload_status_clear.set(None);
                                                });
                                            });
                                        }
                                    },
                                }
                                if is_uploading() {
                                    "Uploading..."
                                } else {
                                    "📂 Browse Files"
                                }
                            }
                        }

                        // Upload status
                        if let Some(status) = upload_status() {
                            p {
                                class: "text-xs mt-1 truncate",
                                class: if status.starts_with("✓") { "text-success" } else if status.starts_with("✗") { "text-error" } else { "text-info" },
                                "{status}"
                            }
                        }
                    }

                    // Document list - scrollable
                    div {
                        class: "flex-1 overflow-y-auto min-h-0",

                        div {
                            class: "p-2",

                            div {
                                class: "flex justify-between items-center mb-1",
                                span {
                                    class: "text-xs text-base-content/70",
                                    "Indexed ({documents().len()})"
                                }
                                button {
                                    class: "btn btn-ghost btn-xs",
                                    onclick: refresh_documents,
                                    "↻"
                                }
                            }

                            if documents().is_empty() {
                                p {
                                    class: "text-xs text-base-content/50 italic",
                                    "No documents yet"
                                }
                            } else {
                                ul {
                                    class: "space-y-0.5",
                                    for doc in documents() {
                                        li {
                                            class: "flex items-center justify-between gap-2 text-xs py-0.5 px-1 hover:bg-base-200 rounded",
                                            title: "{doc}",
                                            span { class: "truncate", "📄 {doc}" }
                                            button {
                                                class: "btn btn-ghost btn-xs",
                                                title: "Delete",
                                                onclick: move |_| {
                                                    let doc = doc.clone();
                                                    spawn(async move {
                                                        // Simple confirmation
                                                        let ok = web_sys::window()
                                                            .and_then(|w| {
                                                                w.confirm_with_message(&format!(
                                                                    "Delete '{}' ?",
                                                                    doc
                                                                ))
                                                                .ok()
                                                            })
                                                            .unwrap_or(false);
                                                        if !ok {
                                                            return;
                                                        }

                                                        let _ = api::delete_document(&doc).await;
                                                        if let Ok(mut resp) = api::list_documents().await {
                                                            resp.documents.sort();
                                                            documents.set(resp.documents);
                                                        }
                                                    });
                                                },
                                                "🗑"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Main chat area - takes remaining space
            div {
                class: "flex-1 flex flex-col min-w-0 h-full overflow-hidden relative",

                // Error display
                if let Some(err) = error_msg() {
                    div {
                        class: "alert alert-error mx-2 mt-2 py-2 text-sm flex-shrink-0",
                        span { "{err}" }
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: move |_| error_msg.set(None),
                            "✕"
                        }
                    }
                }

                // Runtime suspended banner (during bulk uploads)
                if runtime_suspended().0 {
                    div {
                        class: "alert mx-2 mt-2 py-2 text-sm flex-shrink-0",
                        style: "background-color: rgba(2, 107, 124, 0.12); border: 1px solid #026B7C;",
                        div { class: "flex flex-col gap-1",
                            span { class: "font-semibold text-white", "Runtime temporarily stopped" }
                            span { class: "text-xs text-gray-200",
                                "The LLM runtime is stopped while documents are uploading to free resources for indexing. Chat will be available again when the upload finishes."
                            }
                        }
                    }
                }

                // Messages area - scrollable, takes remaining space
                // pb-32 provides space for the fixed input box at bottom
                div {
                    class: "flex-1 overflow-y-auto min-h-0 p-2 sm:p-4 pb-32",

                    // Messages container
                    div {
                        class: "max-w-4xl mx-auto space-y-3",

                        for msg in messages() {
                            div {
                                class: if msg.role == "user" { "chat chat-end" } else { "chat chat-start" },

                                div {
                                    class: if msg.role == "user" {
                                        "chat-bubble chat-bubble-primary text-sm sm:text-base"
                                    } else {
                                        "chat-bubble text-sm sm:text-base"
                                    },
                                    style: "white-space: pre-wrap;",
                                    "{msg.content}"
                                }

                                // Show RAG context if available
                                if let Some(ctx) = &msg.context {
                                    div {
                                        class: "chat-footer opacity-50 text-xs mt-1",
                                        details {
                                            summary { class: "cursor-pointer", "📚 Sources used" }
                                            pre {
                                                class: "mt-1 p-2 bg-base-200 rounded text-xs whitespace-pre-wrap",
                                                "{ctx}"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Loading indicator
                        if is_loading() {
                            div {
                                class: "chat chat-start",
                                div {
                                    class: "chat-bubble",
                                    span { class: "loading loading-dots loading-sm" }
                                }
                            }
                        }

                        // Feedback bar for last response
                        if !last_response().is_empty() && !last_response_rated() && !is_loading() {
                            div {
                                class: "flex justify-center items-center gap-3 py-2 mt-2 bg-base-200 rounded-lg",
                                span {
                                    class: "text-sm text-base-content/70",
                                    "Rate this response:"
                                }
                                button {
                                    class: "btn btn-sm btn-ghost text-success hover:bg-success/20",
                                    title: "Good response - save for training",
                                    onclick: move |_| {
                                        let q = last_query();
                                        let r = last_response();
                                        let c = last_context();
                                        let m = chat_mode();
                                        spawn(async move {
                                            let feedback = api::TrainingFeedbackRequest {
                                                query: q,
                                                response: r,
                                                context: c,
                                                quality_score: 5,
                                                conversation_id: None,
                                                mode: Some(m),
                                                model: None,
                                            };
                                            match api::submit_training_feedback(feedback).await {
                                                Ok(resp) => {
                                                    if resp.status == "collected" {
                                                        last_response_rated.set(true);
                                                        feedback_status.set(Some("👍 Saved!".to_string()));
                                                    } else {
                                                        feedback_status.set(Some(resp.message));
                                                    }
                                                }
                                                Err(e) => {
                                                    feedback_status.set(Some(format!("Error: {}", e)));
                                                }
                                            }
                                            gloo_timers::future::TimeoutFuture::new(2000).await;
                                            feedback_status.set(None);
                                        });
                                    },
                                    "👍 Good"
                                }
                                button {
                                    class: "btn btn-sm btn-ghost text-error hover:bg-error/20",
                                    title: "Poor response - save for training",
                                    onclick: move |_| {
                                        let q = last_query();
                                        let r = last_response();
                                        let c = last_context();
                                        let m = chat_mode();
                                        spawn(async move {
                                            let feedback = api::TrainingFeedbackRequest {
                                                query: q,
                                                response: r,
                                                context: c,
                                                quality_score: 2,
                                                conversation_id: None,
                                                mode: Some(m),
                                                model: None,
                                            };
                                            match api::submit_training_feedback(feedback).await {
                                                Ok(resp) => {
                                                    if resp.status == "collected" {
                                                        last_response_rated.set(true);
                                                        feedback_status.set(Some("👎 Noted".to_string()));
                                                    } else {
                                                        feedback_status.set(Some(resp.message));
                                                    }
                                                }
                                                Err(e) => {
                                                    feedback_status.set(Some(format!("Error: {}", e)));
                                                }
                                            }
                                            gloo_timers::future::TimeoutFuture::new(2000).await;
                                            feedback_status.set(None);
                                        });
                                    },
                                    "👎 Poor"
                                }
                                // Show feedback status
                                if let Some(status) = feedback_status() {
                                    span {
                                        class: "text-sm font-medium",
                                        "{status}"
                                    }
                                }
                            }
                        }

                        // Show "Rated" confirmation
                        if last_response_rated() && !is_loading() {
                            div {
                                class: "flex justify-center items-center py-2 mt-2",
                                span {
                                    class: "text-sm text-success",
                                    "✓ Response rated - thank you!"
                                }
                            }
                        }
                    }
                }

            }

            // Input area - fixed at bottom, centered on viewport
            div {
                class: "fixed bottom-0 inset-x-0 p-2 sm:p-3",

                // Container for model dropdown + input box
                div {
                    class: "mx-auto mb-2 flex items-center gap-2",
                    style: "width: 48rem; max-width: 95vw; margin-left: calc(50% - 24rem + 1cm);",

                    // Model dropdown - left of input
                    div {
                        class: "flex-shrink-0",
                        select {
                            class: "select select-bordered select-sm rounded-xl text-xs",
                            style: "min-width: 8rem; max-width: 12rem; height: 2.5rem; background-color: #1f2937; border-color: #374151; color: white;",
                            value: "{selected_model}",
                            disabled: models_loading() || is_loading(),
                            onchange: move |evt| {
                                let new_model = evt.value();
                                selected_model.set(new_model.clone());
                                // Save model selection to backend
                                spawn(async move {
                                    // Fetch current hardware config
                                    if let Ok(resp) = api::fetch_hardware_config().await {
                                        let mut config = resp.config;
                                        config.model = new_model;
                                        // Save updated config
                                        let _ = api::commit_hardware_config(&config).await;
                                    }
                                });
                            },

                            if models_loading() {
                                option { value: "", "Loading..." }
                            } else if available_models().is_empty() {
                                option { value: "{selected_model}", "{selected_model}" }
                            } else {
                                for model in available_models() {
                                    option {
                                        value: "{model.name}",
                                        selected: model.name == selected_model() || model.is_active,
                                        if model.is_active {
                                            "⚡ {friendly_model_name(&model.name)}"
                                        } else {
                                            "{friendly_model_name(&model.name)}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Input box with Send button inside
                    div {
                        class: "relative flex-1",

                        input {
                            class: "input input-bordered text-sm sm:text-base pl-4 pr-14 rounded-2xl focus:border-[#3b82f6] focus:outline-none focus:ring-1 focus:ring-[#3b82f6] w-full",
                            style: "height: 5rem;",
                            r#type: "text",
                            placeholder: "Type a message...",
                            value: "{input_text}",
                            oninput: move |evt| input_text.set(evt.value()),
                            onkeypress: on_keypress,
                            disabled: is_loading(),
                        }

                        // Send button - inside input on the right
                        button {
                            class: "absolute right-2 top-1/2 -translate-y-1/2 btn btn-ghost btn-circle hover:bg-transparent z-10",
                            onclick: send_message,
                            disabled: is_loading() || input_text().trim().is_empty(),

                            if is_loading() {
                                span { class: "loading loading-spinner loading-sm" }
                            } else {
                                img {
                                    src: asset!("/assets/styling/send-button.svg"),
                                    alt: "Send",
                                    class: "w-10 h-10",
                                }
                            }
                        }
                    }
                }

                // Stop button row - only visible when loading
                if is_loading() {
                    div {
                        class: "flex justify-center gap-2 items-center",
                        button {
                            class: "btn btn-outline rounded-3xl px-6 text-sm text-white border border-red-400 hover:bg-red-700",
                            onclick: stop_request,
                            "Stop"
                        }
                    }
                }
            }

            // Info Panel Overlay
            if show_info().0 {
                div {
                    class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                    onclick: move |_| show_info.set(ShowRagInfo(false)),

                    div {
                        class: "bg-base-100 rounded-lg p-6 max-w-md mx-4 shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),

                        h3 { class: "text-lg font-bold mb-4", "ℹ️ RAG Information" }

                        div {
                            class: "space-y-3 text-sm",

                            p {
                                strong { "What is RAG?" }
                                br {}
                                "Retrieval-Augmented Generation searches your uploaded documents to provide context-aware answers."
                            }

                            p {
                                strong { "How it works:" }
                                ul {
                                    class: "list-disc list-inside mt-1",
                                    li { "Upload documents (PDF, text, markdown, HTML, JSON, XML) or code files" }
                                    li { "Indexing runs (extract → chunk → embed → store) so your documents become searchable" }
                                    li { "Ask questions in the chat" }
                                    li { "Relevant content is found and sent to the LLM runtime" }
                                    li { "Get answers grounded in your documents" }
                                }
                            }

                            p {
                                strong { "During bulk uploads:" }
                                br {}
                                "When you add documents, the app will automatically stop the LLM runtime to free CPU/RAM for indexing. While the runtime is stopped, chat/LLM answers are not available. Start the runtime again when the upload is finished to re-enable chat."
                            }

                            p {
                                strong { "Toggle RAG:" }
                                br {}
                                "Use the RAG switch to enable/disable document search. When off, the LLM answers from its training only."
                            }
                        }

                        button {
                            class: "btn btn-primary btn-sm mt-4 w-full",
                            onclick: move |_| show_info.set(ShowRagInfo(false)),
                            "Got it!"
                        }
                    }
                }
            }

            // Help Modal Overlay
            if show_help_modal() {
                div {
                    class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                    onclick: move |_| show_help_modal.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4",
                        style: "margin-top: -3cm;",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "📖 Help" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_help_modal.set(false),
                                "✕"
                            }
                        }

                        pre {
                            class: "text-xs font-mono whitespace-pre-wrap bg-base-200 p-3 rounded-lg",
                            "{help_content}"
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_help_modal.set(false),
                            "Close"
                        }
                    }
                }
            }

            // RAG Mode Info Modal
            if show_rag_info() {
                div {
                    class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                    onclick: move |_| show_rag_info.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4 max-w-md",
                        style: "margin-top: -3cm;",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "📚 RAG Mode" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_rag_info.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-2",
                            p { class: "font-semibold text-blue-400", "Retrieval-Augmented Generation" }
                            p { "Searches your uploaded documents and returns relevant information directly from them." }
                            div {
                                class: "bg-base-200 p-2 rounded mt-2",
                                p { class: "font-medium", "Settings:" }
                                ul {
                                    class: "text-xs list-disc list-inside mt-1 space-y-1",
                                    li { "Temperature: 0.2 (factual)" }
                                    li { "Top-K: 15 (focused)" }
                                    li { "Max tokens: 512" }
                                }
                            }
                            p { class: "text-xs text-base-content/60 mt-2", "Best for: Factual Q&A from your documents" }
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_rag_info.set(false),
                            "Close"
                        }
                    }
                }
            }

            // LLM Mode Info Modal
            if show_llm_info() {
                div {
                    class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                    onclick: move |_| show_llm_info.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4 max-w-md",
                        style: "margin-top: -3cm;",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "🤖 LLM Mode" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_llm_info.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-2",
                            p { class: "font-semibold text-green-400", "Pure Language Model" }
                            p { "Uses the AI model directly without searching documents. Relies on the model's training knowledge." }
                            div {
                                class: "bg-base-200 p-2 rounded mt-2",
                                p { class: "font-medium", "Settings:" }
                                ul {
                                    class: "text-xs list-disc list-inside mt-1 space-y-1",
                                    li { "Temperature: 0.7 (creative)" }
                                    li { "Top-K: 40 (diverse)" }
                                    li { "Max tokens: 1024" }
                                }
                            }
                            p { class: "text-xs text-base-content/60 mt-2", "Best for: General questions, creative tasks, coding help" }
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_llm_info.set(false),
                            "Close"
                        }
                    }
                }
            }

            // Hybrid Mode Info Modal
            if show_hybrid_info() {
                div {
                    class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                    onclick: move |_| show_hybrid_info.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4 max-w-md",
                        style: "margin-top: -3cm;",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "⚡ Hybrid Mode" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_hybrid_info.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-2",
                            p { class: "font-semibold text-purple-400", "Best of Both Worlds" }
                            p { "First searches your documents. If relevant content is found, uses it as context for the LLM. Falls back to pure LLM if no documents match." }
                            div {
                                class: "bg-base-200 p-2 rounded mt-2",
                                p { class: "font-medium", "Settings:" }
                                ul {
                                    class: "text-xs list-disc list-inside mt-1 space-y-1",
                                    li { "Temperature: 0.4 (balanced)" }
                                    li { "Top-K: 30 (moderate)" }
                                    li { "Max tokens: 768" }
                                }
                            }
                            p { class: "text-xs text-base-content/60 mt-2", "Best for: Most use cases - grounded answers with AI enhancement" }
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_hybrid_info.set(false),
                            "Close"
                        }
                    }
                }
            }

            // Backend Info Modal
            if show_backend_info() {
                div {
                    class: "fixed inset-0 bg-black/60 flex items-center justify-center overflow-y-auto pointer-events-auto",
                    style: "z-index: 1000;",
                    onclick: move |_| show_backend_info.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4 max-w-lg my-4 pointer-events-auto",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "Inference backend" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_backend_info.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-2",
                            p { "Select the runtime that executes prompts (local llama.cpp, vLLM, OpenAI, etc.)." }
                            p { "Switching backend clears the model name so you can pick a compatible artifact." }
                        }
                    }
                }
            }

            // KV Cache Info Modal
            if show_cache_info() {
                div {
                    class: "fixed inset-0 bg-black/60 flex items-center justify-center overflow-y-auto pointer-events-auto",
                    style: "z-index: 1000;",
                    onclick: move |_| show_cache_info.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4 max-w-lg my-4 pointer-events-auto",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "⚡ KV Cache" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_cache_info.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-2",

                            // What is KV Cache
                            div {
                                class: "bg-base-200 p-2 rounded border border-base-300",
                                h4 { class: "text-xs font-semibold text-base-content/70", "What is KV Cache?" }
                                div {
                                    class: "text-xs mt-1 space-y-1",
                                    p {
                                        strong { "KV" }
                                        " stands for "
                                        strong { "Key-Value" }
                                        ". In the transformer attention mechanism:"
                                    }
                                    ul {
                                        class: "list-disc ml-4 space-y-1",
                                        li {
                                            strong { "K (Key)" }
                                            " and "
                                            strong { "V (Value)" }
                                            " are two of the three matrices computed during self-attention (the third being "
                                            strong { "Q - Query" }
                                            ")"
                                        }
                                        li { "When generating text token-by-token, the model recomputes attention for all previous tokens" }
                                        li {
                                            "The "
                                            strong { "KV Cache" }
                                            " stores the Key and Value vectors from previous tokens so they don't need to be recomputed"
                                        }
                                    }
                                }
                            }

                            // Link to attention explanation
                            div {
                                class: "bg-base-200 p-2 rounded border border-base-300",
                                h4 { class: "text-xs font-semibold text-base-content/70", "Need a refresher?" }
                                p {
                                    class: "text-xs mt-1",
                                    a {
                                        class: "text-blue-400 underline hover:text-blue-300 cursor-pointer",
                                        onclick: move |_| {
                                            show_attention_details.set(true);
                                            show_cache_info.set(false);
                                        },
                                        "What is attention?"
                                    }
                                }
                            }

                            // What changes
                            div {
                                class: "bg-base-200 p-2 rounded",
                                p { class: "font-medium", "What changes:" }
                                div {
                                    class: "text-xs mt-1 space-y-1",
                                    p {
                                        span {
                                            class: "text-red-400 font-medium",
                                            "OFF: "
                                        }
                                        span {
                                            class: "text-blue-300 underline cursor-pointer",
                                            onclick: move |_| show_kv_details.set(true),
                                            "K/V"
                                        }
                                        " recomputed each request."
                                    }
                                    p {
                                        span {
                                            class: "text-green-400 font-medium",
                                            "ON: "
                                        }
                                        span {
                                            class: "text-blue-300 underline cursor-pointer",
                                            onclick: move |_| show_kv_details.set(true),
                                            "K/V"
                                        }
                                        " cached; only new tokens computed."
                                    }
                                }
                            }

                            // Example
                            div {
                                class: "bg-base-200 p-2 rounded",
                                p { class: "font-medium", "Example (follow-up question):" }
                                div {
                                    class: "text-xs mt-1 space-y-1",
                                    p {
                                        span { class: "text-red-400", "Without: " }
                                        "5000 tokens computed twice"
                                    }
                                    p {
                                        span { class: "text-green-400", "With: " }
                                        "5000 tokens cached, only new tokens computed"
                                    }
                                }
                                p {
                                    class: "text-[11px] text-base-content/60 mt-1",
                                    "KV cache is just an attention shortcut—see the "
                                    a {
                                        class: "text-blue-400 underline hover:text-blue-300 cursor-pointer",
                                        onclick: move |_| {
                                            show_attention_details.set(true);
                                            show_cache_info.set(false);
                                        },
                                        "attention explainer"
                                    }
                                    " for the math."
                                }
                            }

                            // Per backend
                            div {
                                class: "bg-base-200 p-2 rounded",
                                p { class: "font-medium", "Per Backend:" }
                                ul {
                                    class: "text-xs list-disc list-inside mt-1 space-y-1",
                                    li { "Ollama: /api/chat + keep_alive" }
                                    li { "OpenAI: Structured messages for prefix caching" }
                                    li { "Anthropic: cache_control hints (beta)" }
                                }
                                p { class: "text-xs text-green-400 mt-1", "✓ All backends supported" }
                            }

                            // Why disabled by default
                            div {
                                class: "bg-base-200 p-2 rounded",
                                p { class: "font-medium text-yellow-400", "Why Disabled by Default:" }
                                ul {
                                    class: "text-xs mt-1 space-y-1",
                                    li {
                                        span { class: "font-medium", "Not universally beneficial: " }
                                        "Short prompts (<1024 tokens) don't benefit"
                                    }
                                    li {
                                        span { class: "font-medium", "Resource usage: " }
                                        "KV cache consumes GPU/CPU memory"
                                    }
                                    li {
                                        span { class: "font-medium", "Debugging simplicity: " }
                                        "Stateless requests are easier to debug"
                                    }
                                    li {
                                        span { class: "font-medium", "Cost for cloud: " }
                                        "Anthropic charges extra to write to cache"
                                    }
                                    li {
                                        span { class: "font-medium", "Cache misses: " }
                                        "First request has no benefit; varied prompts have low hit rates"
                                    }
                                    li {
                                        span { class: "font-medium", "Different API behavior: " }
                                        button {
                                            class: "text-blue-400 underline hover:text-blue-300 cursor-pointer",
                                            onclick: move |_| {
                                                show_api_behavior.set(true);
                                                show_cache_info.set(false);
                                                show_kv_details.set(false);
                                            },
                                            "Ollama: /api/chat vs /api/generate have different semantics →"
                                        }
                                    }
                                }
                            }

                            // When to enable
                            div {
                                class: "bg-base-200 p-2 rounded",
                                p { class: "font-medium text-green-400", "When to Enable:" }
                                ul {
                                    class: "text-xs mt-1 space-y-1",
                                    li {
                                        span { class: "font-medium", "High-volume apps: " }
                                        "Many similar requests benefit from cache reuse"
                                    }
                                    li {
                                        span { class: "font-medium", "Long system prompts: " }
                                        "2000+ token system prompts get cached"
                                    }
                                    li {
                                        span { class: "font-medium", "RAG with stable context: " }
                                        "Same documents retrieved repeatedly"
                                    }
                                    li {
                                        span { class: "font-medium", "Cost-sensitive production: " }
                                        "Up to 10x cheaper on cloud API costs"
                                    }
                                    li {
                                        span { class: "font-medium", "Latency-sensitive: " }
                                        "Up to 85% faster for long cached prompts"
                                    }
                                }
                            }
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_cache_info.set(false),
                            "Close"
                        }
                    }
                }
            }

            // API Behavior Modal (linked from KV Cache modal)
            if show_api_behavior() {
                div {
                    class: "fixed inset-0 bg-black/65 flex items-center justify-center overflow-y-auto pointer-events-auto",
                    style: "z-index: 1200;",
                    onclick: move |_| show_api_behavior.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-4 max-w-2xl my-4 pointer-events-auto",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "/api/generate vs /api/chat" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_api_behavior.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-3",

                            // Use cases for /api/generate
                            div {
                                class: "bg-base-200 p-3 rounded",
                                h4 { class: "font-semibold", "Use cases for /api/generate" }
                                ul {
                                    class: "list-disc list-inside text-xs mt-2 space-y-1",
                                    li { strong { "Single-shot completions — " } "One-off text generation without conversation context" }
                                    li { strong { "Custom prompt templates — " } "When you want full control over the exact prompt format, bypassing the model's built-in chat template" }
                                    li { strong { "Text completion tasks — " } "Finishing a sentence, code completion, fill-in-the-blank" }
                                    li { strong { "Embeddings workaround — " } "Some setups use it with \"raw\": true for specific tokenization needs" }
                                    li { strong { "Legacy/simpler integrations — " } "When you just need \"text in, text out\"" }
                                    li { strong { "Benchmarking — " } "Testing raw model performance without chat template overhead" }
                                }
                            }

                            // Use cases for /api/chat
                            div {
                                class: "bg-base-200 p-3 rounded",
                                h4 { class: "font-semibold", "Use cases for /api/chat" }
                                ul {
                                    class: "list-disc list-inside text-xs mt-2 space-y-1",
                                    li { strong { "Multi-turn conversations — " } "Chatbots, assistants, agentic loops with KV cache reuse" }
                                    li { strong { "Role-based prompting — " } "Clean separation of system, user, assistant" }
                                    li { strong { "Model-native formatting — " } "Correct chat template applied automatically" }
                                    li { strong { "Agentic workflows — " } "Tool use loops with fast back-and-forth" }
                                    li { strong { "Lower latency on follow-ups — " } "Subsequent turns skip reprocessing history" }
                                }
                            }

                            // Comparison table
                            div {
                                class: "overflow-x-auto",
                                table {
                                    class: "table table-xs w-full",
                                    thead {
                                        tr {
                                            th { class: "text-left", "Aspect" }
                                            th { class: "text-left text-red-400", "/api/generate" }
                                            th { class: "text-left text-green-400", "/api/chat" }
                                        }
                                    }
                                    tbody {
                                        tr {
                                            td { class: "font-medium", "Request format" }
                                            td { "Single prompt string" }
                                            td { "Array of messages" }
                                        }
                                        tr {
                                            td { class: "font-medium", "System prompt" }
                                            td { "Separate system field" }
                                            td { "Message with role: \"system\"" }
                                        }
                                        tr {
                                            td { class: "font-medium", "Context handling" }
                                            td { "Embedded in prompt" }
                                            td { "Separate user message + assistant ack" }
                                        }
                                        tr {
                                            td { class: "font-medium", "Cache behavior" }
                                            td { "None" }
                                            td { "Prefix matching on message array" }
                                        }
                                        tr {
                                            td { class: "font-medium", "Model memory" }
                                            td { "Unloads after request" }
                                            td { "keep_alive keeps loaded" }
                                        }
                                        tr {
                                            td { class: "font-medium", "Response format" }
                                            td { code { class: "text-xs", "{{ \"response\": \"...\" }}" } }
                                            td { code { class: "text-xs", "{{ \"message\": {{ \"content\": \"...\" }} }}" } }
                                        }
                                    }
                                }
                            }

                            // Synthetic assistant message warning
                            div {
                                class: "bg-yellow-900/30 border border-yellow-600/50 p-3 rounded",
                                p { class: "font-medium text-yellow-400 mb-2", "⚠️ The \"Synthetic\" Assistant Message" }
                                p { class: "text-xs mb-2", "Notice this in the chat format:" }
                                pre {
                                    class: "bg-base-300 p-2 rounded text-xs overflow-x-auto",
                                    code {
                                        "{{ \"role\": \"assistant\", \"content\": \"I'll use this context to help answer your questions.\" }}"
                                    }
                                }
                                p { class: "text-xs mt-2",
                                    "This is "
                                    span { class: "font-medium", "injected automatically" }
                                    " to help maintain cache alignment. It's not a real response - it's a trick to make the message prefix more stable for caching."
                                }
                                p { class: "text-xs mt-2 text-yellow-300",
                                    "This can affect behavior because the model \"sees\" this as part of the conversation history, potentially influencing its responses."
                                }
                            }
                        }

                        div {
                            class: "flex gap-2 mt-3",
                            button {
                                class: "btn btn-ghost btn-xs flex-1",
                                onclick: move |_| {
                                    show_api_behavior.set(false);
                                },
                                "← Back to KV Cache"
                            }
                            button {
                                class: "btn btn-primary btn-xs flex-1",
                                onclick: move |_| {
                                    show_api_behavior.set(false);
                                    show_cache_info.set(false);
                                },
                                "Close All"
                            }
                        }
                    }
                }
            }

            // KV Fundamentals Modal
            if show_kv_details() {
                div {
                    class: "fixed inset-0 bg-black/70 flex items-center justify-center overflow-y-auto pointer-events-auto",
                    style: "z-index: 1500;",
                    onclick: move |_| show_kv_details.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-5 max-w-lg my-6 pointer-events-auto",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "K/V Fundamentals" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_kv_details.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-2",
                            p {
                                "K (key) and V (value) vectors are core components of the attention mechanism, most famously in transformers."
                            }
                            p { "Here's the intuition:" }
                            ul {
                                class: "list-disc list-inside text-xs space-y-1",
                                li { strong { "Q (query) — " } "\"what am I looking for?\"" }
                                li { strong { "K (key) — " } "\"what do I contain?\" (used to compute relevance)" }
                                li { strong { "V (value) — " } "\"what information do I actually pass along?\"" }
                            }
                            p { "The attention score is computed by taking the dot product of Q with all K vectors (measuring similarity), then using those scores to create a weighted sum of the V vectors." }
                            p {
                                class: "text-xs",
                                a {
                                    class: "text-blue-400 underline hover:text-blue-300 cursor-pointer",
                                    onclick: move |_| {
                                        show_attention_details.set(true);
                                        show_kv_details.set(false);
                                        show_cache_info.set(false);
                                    },
                                    "Dive deeper into attention"
                                }
                            }
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_kv_details.set(false),
                            "Close"
                        }
                    }
                }
            }

            // Attention Fundamentals Modal
            if show_attention_details() {
                div {
                    class: "fixed inset-0 bg-black/80 flex items-center justify-center overflow-y-auto pointer-events-auto",
                    style: "z-index: 2147483646;",
                    onclick: move |_| show_attention_details.set(false),

                    div {
                        class: "bg-base-100 rounded-lg mx-4 shadow-xl p-5 max-w-3xl my-6 pointer-events-auto",
                        style: "z-index: 2147483647;",
                        onclick: move |evt| evt.stop_propagation(),

                        div {
                            class: "flex justify-between items-center mb-3",
                            h3 { class: "text-base font-bold", "Attention 101" }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| show_attention_details.set(false),
                                "✕"
                            }
                        }

                        div {
                            class: "text-sm space-y-3",
                            p {
                                "The attention mechanism lets a model dynamically weigh how much each token in a sequence should influence the representation of every other token."
                            }
                            h4 { class: "font-semibold text-base-content", "Core idea" }
                            p {
                                "For each token, you compute three vectors from its embedding: a query (what am I looking for?), a key (what do I contain?), and a value (what information do I carry?)."
                            }
                            p {
                                "Attention scores are computed by taking the dot product of a query with all keys, then softmaxing to get weights, and finally using those weights to sum the values."
                            }
                            h4 { class: "font-semibold text-base-content", "Multi-head attention" }
                            p {
                                "Rather than computing a single attention function, transformers run several attention \"heads\" in parallel, each with its own learned Q/K/V projections."
                            }
                            p {
                                "This lets the model attend to information from different representation subspaces—one head might capture syntactic relationships, another semantic similarity, another positional patterns. The outputs are concatenated and projected back down."
                            }
                            h4 { class: "font-semibold text-base-content", "Self-attention vs cross-attention" }
                            p {
                                "In self-attention (used in both encoder and decoder), Q, K, and V all come from the same sequence—each token attends to all others."
                            }
                            p {
                                "In cross-attention (decoder attending to encoder outputs), queries come from the decoder while keys and values come from the encoder, letting the model ground its generation in the input."
                            }
                            h4 { class: "font-semibold text-base-content", "Why it matters" }
                            p {
                                "Unlike RNNs, attention connects any two positions with O(1) sequential operations, making long-range dependencies much easier to learn."
                            }
                            p {
                                "The trade-off is O(n²) memory in sequence length, which drives much of the work on efficient attention variants like sparse attention, linear attention, or sliding window approaches."
                            }
                        }

                        button {
                            class: "btn btn-primary btn-xs w-full mt-3",
                            onclick: move |_| show_attention_details.set(false),
                            "Close"
                        }
                    }
                }
            }

        }

        if show_delete_docs_modal() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/70",
                onclick: move |_| show_delete_docs_modal.set(false),
                div {
                    class: "bg-gray-900 border border-gray-700 rounded-lg w-[90vw] max-w-lg max-h-[90vh] shadow-xl flex flex-col",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex justify-between items-center px-4 py-3 border-b border-gray-800",
                        h2 { class: "text-lg font-semibold text-white", "Delete Documents" }
                        button { class: "btn btn-ghost btn-xs", onclick: move |_| show_delete_docs_modal.set(false), "✕" }
                    }
                    div { class: "px-4 py-2 text-sm text-gray-300", "Select the documents you want to remove from the knowledge base." }
                    div { class: "px-4 text-sm text-red-300", "This only deletes the indexed copy. Original files remain wherever they were stored." }
                    div { class: "flex-1 overflow-y-auto p-4 space-y-2",
                        if documents().is_empty() {
                            p { class: "text-gray-500 text-sm", "No documents indexed." }
                        } else {
                            for doc in documents() {
                                label { class: "flex items-center gap-2 text-sm text-white cursor-pointer bg-gray-800/60 px-3 py-2 rounded border border-gray-700",
                                    input {
                                        r#type: "checkbox",
                                        checked: selected_documents().contains(&doc),
                                        onclick: {
                                            let doc_clone = doc.clone();
                                            move |_| toggle_doc_selection(doc_clone.clone())
                                        },
                                    }
                                    span { "{doc}" }
                                }
                            }
                        }
                    }
                    if let Some(status) = delete_docs_status() {
                        div { class: "px-4 text-sm text-gray-300", "{status}" }
                    }
                    div { class: "flex justify-between items-center gap-2 p-4 border-t border-gray-800",
                        button { class: "btn btn-ghost btn-sm", onclick: move |_| {
                                selected_documents.set(Vec::new());
                                show_delete_docs_modal.set(false);
                            }, "Cancel" }
                        button {
                            class: "btn btn-error btn-sm",
                            disabled: deleting_documents() || selected_documents().is_empty(),
                            onclick: move |_| {
                                if deleting_documents() || selected_documents().is_empty() {
                                    return;
                                }
                                deleting_documents.set(true);
                                delete_docs_status.set(Some("Deleting…".to_string()));
                                let docs = selected_documents();
                                spawn(async move {
                                    let mut errors = vec![];
                                    for name in docs.iter() {
                                        match api::delete_document(name).await {
                                            Ok(_) => {}
                                            Err(e) => errors.push(format!("{}: {}", name, e)),
                                        }
                                    }
                                    match api::list_documents().await {
                                        Ok(mut resp) => {
                                            resp.documents.sort();
                                            documents.set(resp.documents);
                                        }
                                        Err(e) => upload_status.set(Some(format!("Failed to load: {}", e))),
                                    }
                                    if errors.is_empty() {
                                        delete_docs_status.set(Some("✓ Deleted".to_string()));
                                    } else {
                                        delete_docs_status.set(Some(format!("Issues: {}", errors.join(", "))));
                                    }
                                    selected_documents.set(Vec::new());
                                    deleting_documents.set(false);
                                });
                            },
                            "Delete Selected"
                        }
                    }
                }
            }
        }

        if show_delete_memories_modal() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/70",
                onclick: move |_| show_delete_memories_modal.set(false),
                div {
                    class: "bg-gray-900 border border-gray-700 rounded-lg w-[95vw] max-w-3xl max-h-[90vh] shadow-xl flex flex-col",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex justify-between items-center px-4 py-3 border-b border-gray-800",
                        h2 { class: "text-lg font-semibold text-white", "Delete RAG Memories" }
                        button { class: "btn btn-ghost btn-xs", onclick: move |_| show_delete_memories_modal.set(false), "✕" }
                    }
                    div { class: "px-4 py-2 text-sm text-gray-300", "Choose memory entries to remove permanently." }
                    if let Some(err) = memory_error() {
                        div { class: "px-4 text-sm text-red-400", "{err}" }
                    }
                    div { class: "flex-1 overflow-y-auto p-4 space-y-2",
                        if memories_loading() {
                            div { class: "text-center text-gray-400", "Loading memories…" }
                        } else if rag_memories().is_empty() {
                            div { class: "text-gray-500 text-sm", "No memories found." }
                        } else {
                            for mem in rag_memories() {
                                div { class: "bg-gray-800/60 rounded border border-gray-700 p-3 space-y-1",
                                    div { class: "flex items-center justify-between",
                                        label { class: "flex items-center gap-2 text-sm text-white cursor-pointer",
                                            input {
                            r#type: "checkbox",
                            checked: selected_memories().contains(&mem.id),
                            onclick: {
                                let id = mem.id;
                                move |_| toggle_memory_selection(id)
                            }
                        }
                                            span { class: "font-semibold", "#{mem.id} · {mem.memory_type}" }
                                        }
                                        span { class: "text-xs text-gray-500", "{mem.timestamp}" }
                                    }
                                    p { class: "text-sm text-gray-200 whitespace-pre-wrap", "{mem.content}" }
                                }
                            }
                        }
                    }
                    if let Some(status) = delete_memories_status() {
                        div { class: "px-4 text-sm text-gray-300", "{status}" }
                    }
                    div { class: "flex justify-between items-center gap-2 p-4 border-t border-gray-800",
                        button { class: "btn btn-ghost btn-sm", onclick: move |_| {
                                selected_memories.set(Vec::new());
                                show_delete_memories_modal.set(false);
                            }, "Close" }
                        button {
                            class: "btn btn-error btn-sm",
                            disabled: deleting_memories() || selected_memories().is_empty(),
                            onclick: move |_| {
                                if deleting_memories() || selected_memories().is_empty() {
                                    return;
                                }
                                deleting_memories.set(true);
                                delete_memories_status.set(Some("Deleting…".to_string()));
                                let ids = selected_memories();
                                spawn(async move {
                                    let req = api::DeleteRagRequest {
                                        agent_id: "default".to_string(),
                                        ids,
                                    };
                                    match api::delete_rag_memories(&req).await {
                                        Ok(resp) => {
                                            delete_memories_status.set(Some(format!("✓ Deleted {}", resp.deleted)));
                                            selected_memories.set(Vec::new());
                                            memories_loading.set(true);
                                            memory_error.set(None);
                                            match api::fetch_rag_memories(100).await {
                                                Ok(list) => rag_memories.set(list.memories),
                                                Err(e) => memory_error.set(Some(e)),
                                            }
                                            memories_loading.set(false);
                                        }
                                        Err(e) => delete_memories_status.set(Some(format!("Error: {}", e))),
                                    }
                                    deleting_memories.set(false);
                                });
                            },
                            "Delete Selected"
                        }
                    }
                }
            }
        }

        // File Types Info Modal (outside overflow-hidden container)
        if show_file_types_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_file_types_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-md max-h-[90vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Supported File Types" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_file_types_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 space-y-4",
                        // Documents section
                        div {
                            h3 { class: "font-semibold text-blue-300 mb-2", "Documents" }
                            ul { class: "list-disc list-inside space-y-1 text-gray-400",
                                li { span { class: "text-gray-200", "PDF:" } " .pdf" }
                                li { span { class: "text-gray-200", "Text:" } " .txt, .text" }
                                li { span { class: "text-gray-200", "Markdown:" } " .md, .markdown" }
                                li { span { class: "text-gray-200", "HTML:" } " .html, .htm, .xhtml" }
                                li { span { class: "text-gray-200", "XML:" } " .xml" }
                                li { span { class: "text-gray-200", "JSON:" } " .json" }
                            }
                        }
                        div {
                            class: "p-3 rounded border border-gray-700 bg-gray-900/30",
                            h3 { class: "font-semibold text-cyan-300 mb-1", "Tip for bulk uploads" }
                            p { class: "text-gray-300",
                                "When you add documents, the app will automatically stop the LLM runtime to free CPU/RAM for indexing. While the runtime is stopped, chat/LLM answers are not available. Start the runtime again when the upload is finished to re-enable chat."
                            }
                        }

                        // Code Files section
                        div {
                            h3 { class: "font-semibold text-purple-300 mb-2", "Code Files" }
                            ul { class: "list-disc list-inside space-y-1 text-gray-400",
                                li { span { class: "text-gray-200", "Rust:" } " .rs" }
                                li { span { class: "text-gray-200", "Python:" } " .py, .pyw" }
                                li { span { class: "text-gray-200", "JavaScript:" } " .js, .mjs, .cjs" }
                                li { span { class: "text-gray-200", "TypeScript:" } " .ts, .tsx" }
                                li { span { class: "text-gray-200", "Go:" } " .go" }
                                li { span { class: "text-gray-200", "Java:" } " .java" }
                                li { span { class: "text-gray-200", "C#:" } " .cs" }
                                li { span { class: "text-gray-200", "C/C++:" } " .c, .h, .cpp, .cc, .cxx, .hpp" }
                                li { span { class: "text-gray-200", "Ruby:" } " .rb" }
                                li { span { class: "text-gray-200", "PHP:" } " .php" }
                                li { span { class: "text-gray-200", "Shell:" } " .sh, .bash, .zsh" }
                                li { span { class: "text-gray-200", "SQL:" } " .sql" }
                                li { span { class: "text-gray-200", "YAML:" } " .yaml, .yml" }
                                li { span { class: "text-gray-200", "TOML:" } " .toml" }
                            }
                        }
                    }
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_file_types_info.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }
}
