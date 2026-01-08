use crate::api;
use crate::app::ShowRagInfo;
use dioxus::prelude::*;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Request, RequestInit, RequestMode, Response};

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub context: Option<String>, // RAG context used (if any)
}

const BACKEND_AGENT_URL: &str = "http://127.0.0.1:3010/agent";
const BACKEND_STREAM_URL: &str = "http://127.0.0.1:3010/agent/stream";

/// Check if input is a chat command (starts with /)
fn is_chat_command(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("/help") ||
    trimmed.starts_with("/goal") ||
    trimmed.starts_with("/goals") ||
    trimmed.starts_with("/status") ||
    trimmed.starts_with("/models") ||
    trimmed.starts_with("/clear") ||
    trimmed.starts_with("/focus") ||
    trimmed.starts_with("/unfocus") ||
    trimmed.starts_with("/persona") ||
    trimmed.starts_with("/verbose") ||
    trimmed.starts_with("/brief") ||
    trimmed.starts_with("/run") ||
    trimmed.starts_with("/chain") ||
    trimmed.starts_with("/retry") ||
    trimmed.starts_with("/undo") ||
    trimmed.starts_with("/dry-run") ||
    trimmed.starts_with("/model") ||
    trimmed.starts_with("/temperature") ||
    trimmed.starts_with("/export") ||
    trimmed.starts_with("/import") ||
    trimmed.starts_with("/debug") ||
    trimmed.starts_with("/tokens") ||
    trimmed.starts_with("/forget") ||
    trimmed.starts_with("/history") ||
    trimmed.starts_with("/sources") ||
    trimmed.starts_with("/learn") ||
    trimmed.starts_with("/note") ||
    trimmed.starts_with("/subgoal") ||
    trimmed.starts_with("/pause") ||
    trimmed.starts_with("/resume") ||
    trimmed.starts_with("/abandon") ||
    trimmed.starts_with("/reflect") ||
    trimmed.starts_with("/why")
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
    let selected_model = use_signal(|| "phi:latest".to_string());
    let mut cancel_requested = use_signal(|| false);

    // File upload state
    let mut show_upload_panel = use_signal(|| false);
    let mut documents = use_signal(|| Vec::<String>::new());
    let mut upload_status = use_signal(|| Option::<String>::None);
    let mut is_uploading = use_signal(|| false);

    // Chat mode: "rag", "llm", or "hybrid"
    let mut chat_mode = use_signal(|| "hybrid".to_string());

    // Info panel state (global context)
    let mut show_info = use_context::<Signal<ShowRagInfo>>();

    // Help modal state
    let mut show_help_modal = use_signal(|| false);
    let mut help_content = use_signal(|| String::new());

    // Mode info modal states
    let mut show_rag_info = use_signal(|| false);
    let mut show_llm_info = use_signal(|| false);
    let mut show_hybrid_info = use_signal(|| false);

    // Load documents on mount
    use_effect(move || {
        spawn(async move {
            match api::list_documents().await {
                Ok(resp) => documents.set(resp.documents),
                Err(_) => {} // Silently fail
            }
        });
    });

    // Load active model from hardware config once on mount
    {
        let mut selected_model = selected_model.clone();
        let mut error_signal = error_msg.clone();
        use_future(move || async move {
            // Try to load hardware config (with a quick retry) to keep home page in sync
            let mut last_error = None;
            for attempt in 0..2 {
                match api::fetch_hardware_config().await {
                    Ok(resp) => {
                        let active_model = resp.config.model.trim().to_string();
                        if !active_model.is_empty() {
                            selected_model.set(active_model);
                        }
                        return;
                    }
                    Err(e) => {
                        last_error = Some(e);
                        if attempt == 0 {
                            // Small delay before retrying
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
            }
        });
    }

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
                let request = gloo_net::http::Request::post(BACKEND_AGENT_URL)
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
                                        if user_input.trim() == "/help" || data.response.answer.contains("Available Commands") {
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
                                    error_msg.set(Some(format!("Failed to parse command response: {}", e)));
                                }
                            }
                        } else {
                            error_msg.set(Some(format!("Command failed: HTTP {}", response.status())));
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
            let mut opts = RequestInit::new();
            opts.method("POST");
            opts.mode(RequestMode::Cors);
            opts.body(Some(&JsValue::from_str(&body.to_string())));
            
            let request = Request::new_with_str_and_init(BACKEND_STREAM_URL, &opts).unwrap();
            request.headers().set("Content-Type", "application/json").unwrap();
            
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
                        let reader = body.get_reader().dyn_into::<web_sys::ReadableStreamDefaultReader>().unwrap();
                        let mut accumulated_text = String::new();
                        let mut chunks_used = 0usize;
                        
                        loop {
                            if cancel_flag() {
                                break;
                            }
                            
                            let read_promise = reader.read();
                            match wasm_bindgen_futures::JsFuture::from(read_promise).await {
                                Ok(result) => {
                                    let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                                        .unwrap()
                                        .as_bool()
                                        .unwrap_or(true);
                                    
                                    if done {
                                        break;
                                    }
                                    
                                    let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).unwrap();
                                    let array = js_sys::Uint8Array::new(&value);
                                    let bytes = array.to_vec();
                                    let text = String::from_utf8_lossy(&bytes);
                                    
                                    // Parse SSE events
                                    for line in text.lines() {
                                        if line.starts_with("data: ") {
                                            let json_str = &line[6..];
                                            if let Ok(event) = serde_json::from_str::<serde_json::Value>(json_str) {
                                                if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                                                    match event_type {
                                                        "token" => {
                                                            if let Some(content) = event.get("content").and_then(|v| v.as_str()) {
                                                                accumulated_text.push_str(content);
                                                                // Update the message in place
                                                                if let Some(msg) = messages.write().get_mut(msg_index) {
                                                                    msg.content = accumulated_text.clone();
                                                                }
                                                            }
                                                        }
                                                        "done" => {
                                                            chunks_used = event.get("chunks_used").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                                        }
                                                        "complete" => {
                                                            // Non-streaming response (for RAG mode)
                                                            if let Some(answer) = event.get("answer").and_then(|v| v.as_str()) {
                                                                accumulated_text = answer.to_string();
                                                                if let Some(msg) = messages.write().get_mut(msg_index) {
                                                                    msg.content = accumulated_text.clone();
                                                                }
                                                            }
                                                        }
                                                        "error" => {
                                                            if let Some(err_msg) = event.get("message").and_then(|v| v.as_str()) {
                                                                error_msg.set(Some(err_msg.to_string()));
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
                        if chunks_used > 0 {
                            if let Some(msg) = messages.write().get_mut(msg_index) {
                                msg.context = Some(format!("Used {} chunks from knowledge base", chunks_used));
                            }
                        }
                    }
                }
                Err(e) => {
                    error_msg.set(Some(format!("Request failed: {:?}. Is backend running?", e)));
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
                    let request = gloo_net::http::Request::post(BACKEND_AGENT_URL)
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
                                            if user_input.trim() == "/help" || data.response.answer.contains("Available Commands") {
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
                                        error_msg.set(Some(format!("Failed to parse command response: {}", e)));
                                    }
                                }
                            } else {
                                error_msg.set(Some(format!("Command failed: HTTP {}", response.status())));
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
                let mut opts = RequestInit::new();
                opts.method("POST");
                opts.mode(RequestMode::Cors);
                opts.body(Some(&JsValue::from_str(&body.to_string())));
                
                let request = Request::new_with_str_and_init(BACKEND_STREAM_URL, &opts).unwrap();
                request.headers().set("Content-Type", "application/json").unwrap();
                
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
                            let reader = body.get_reader().dyn_into::<web_sys::ReadableStreamDefaultReader>().unwrap();
                            let mut accumulated_text = String::new();
                            let mut chunks_used = 0usize;
                            
                            loop {
                                if cancel_flag() {
                                    break;
                                }
                                
                                let read_promise = reader.read();
                                match wasm_bindgen_futures::JsFuture::from(read_promise).await {
                                    Ok(result) => {
                                        let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                                            .unwrap()
                                            .as_bool()
                                            .unwrap_or(true);
                                        
                                        if done {
                                            break;
                                        }
                                        
                                        let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).unwrap();
                                        let array = js_sys::Uint8Array::new(&value);
                                        let bytes = array.to_vec();
                                        let text = String::from_utf8_lossy(&bytes);
                                        
                                        for line in text.lines() {
                                            if line.starts_with("data: ") {
                                                let json_str = &line[6..];
                                                if let Ok(event) = serde_json::from_str::<serde_json::Value>(json_str) {
                                                    if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                                                        match event_type {
                                                            "token" => {
                                                                if let Some(content) = event.get("content").and_then(|v| v.as_str()) {
                                                                    accumulated_text.push_str(content);
                                                                    if let Some(msg) = messages.write().get_mut(msg_index) {
                                                                        msg.content = accumulated_text.clone();
                                                                    }
                                                                }
                                                            }
                                                            "done" => {
                                                                chunks_used = event.get("chunks_used").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                                            }
                                                            "complete" => {
                                                                if let Some(answer) = event.get("answer").and_then(|v| v.as_str()) {
                                                                    accumulated_text = answer.to_string();
                                                                    if let Some(msg) = messages.write().get_mut(msg_index) {
                                                                        msg.content = accumulated_text.clone();
                                                                    }
                                                                }
                                                            }
                                                            "error" => {
                                                                if let Some(err_msg) = event.get("message").and_then(|v| v.as_str()) {
                                                                    error_msg.set(Some(err_msg.to_string()));
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
                                    msg.context = Some(format!("Used {} chunks from knowledge base", chunks_used));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error_msg.set(Some(format!("Request failed: {:?}. Is backend running?", e)));
                        messages.write().pop();
                    }
                }

                is_loading.set(false);
            });
        }
    };

    let clear_chat = move |_evt: Event<MouseData>| {
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

    let stop_request = move |_evt: Event<MouseData>| {
        cancel_requested.set(true);
        is_loading.set(false);
        error_msg.set(Some("[INFO] Request cancelled.".to_string()));
    };

    rsx! {
        // Welcome text with RAG toggle - fixed centered relative to full viewport (aligns with header title)
        if messages().is_empty() {
            div {
                class: "fixed inset-x-0 text-center z-10",
                style: "top: 3rem;",
                p { class: "text-sm text-base-content/50", "Type a message or use /help for commands" }

                // Mode selector - centered (offset 3cm lower, scaled 20% bigger)
                div {
                    class: "flex justify-center",
                    style: "margin-top: 5cm; transform: scale(1.2); transform-origin: top center;",
                    div {
                        class: "flex flex-col items-center gap-2",
                        label {
                            class: "font-medium",
                            style: "color: white; font-size: 1.1rem; margin-bottom: 4mm;",
                            "Mode"
                        }
                        div {
                            class: "flex",
                            style: "gap: 1.08rem;",
                            // RAG mode button with info
                            div {
                                class: "flex items-center gap-1",
                                button {
                                    class: "btn btn-sm rounded-lg px-3",
                                    style: if chat_mode() == "rag" {
                                        "background-color:white; border-color:white; color:black; box-shadow:none;"
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
                                    style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                                    onclick: move |_| show_rag_info.set(true),
                                    title: "Info about RAG mode",
                                    svg {
                                        class: "w-5 h-5 text-white",
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                    }
                                }
                            }
                            // LLM mode button with info
                            div {
                                class: "flex items-center gap-1",
                                button {
                                    class: "btn btn-sm rounded-lg px-3",
                                    style: if chat_mode() == "llm" {
                                        "background-color:white; border-color:white; color:black; box-shadow:none;"
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
                                    style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                                    onclick: move |_| show_llm_info.set(true),
                                    title: "Info about LLM mode",
                                    svg {
                                        class: "w-5 h-5 text-white",
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                    }
                                }
                            }
                            // Hybrid mode button with info
                            div {
                                class: "flex items-center gap-1",
                                button {
                                    class: "btn btn-sm rounded-lg px-3",
                                    style: if chat_mode() == "hybrid" {
                                        "background-color:white; border-color:white; color:black; box-shadow:none;"
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
                                    style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                                    onclick: move |_| show_hybrid_info.set(true),
                                    title: "Info about Hybrid mode",
                                    svg {
                                        class: "w-5 h-5 text-white",
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Mode description
                p {
                    class: "text-xs font-medium",
                    style: "color: white; margin-top: calc(0.5rem + 5mm);",
                    match chat_mode().as_str() {
                        "rag" => "RAG mode - searches your documents only",
                        "llm" => "LLM mode - uses AI without document search",
                        "hybrid" => "Hybrid mode - documents + AI fallback",
                        _ => "Select a mode"
                    }
                }

                // Add documents button - centered
                div {
                    class: "flex flex-col items-center mt-4",
                    button {
                        class: "btn rounded-full px-5 text-xl font-bold",
                        style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                        onclick: move |_| show_upload_panel.set(!show_upload_panel()),
                        title: "Toggle documents panel",
                        "+"
                    }
                    span {
                        class: "text-sm mt-1 font-medium",
                        style: "color: white;",
                        "Add documents"
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
                    class: "w-64 lg:w-72 bg-base-100 border-r border-base-300 flex flex-col flex-shrink-0 h-full",

                    // Panel header
                    div {
                        class: "p-2 border-b border-base-300 flex justify-between items-center flex-shrink-0",
                        h2 {
                            class: "font-bold text-sm",
                            "📁 Documents"
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
                            label {
                                class: "block text-xs text-base-content/70 mb-1",
                                "Upload .txt, .md, or .pdf"
                            }
                            input {
                                r#type: "file",
                                class: "file-input file-input-bordered file-input-xs w-full",
                                accept: ".txt,.md,.pdf",
                                disabled: is_uploading(),
                                onchange: move |_evt| {
                                    spawn(async move {
                                        is_uploading.set(true);
                                        upload_status.set(Some("Uploading...".to_string()));

                                        // Get file from event using web_sys
                                        let window = web_sys::window().unwrap();
                                        let document = window.document().unwrap();
                                        let input: web_sys::HtmlInputElement = document
                                            .query_selector("input[type='file']")
                                            .unwrap()
                                            .unwrap()
                                            .dyn_into()
                                            .unwrap();

                                        if let Some(files) = input.files() {
                                            if let Some(file) = files.get(0) {
                                                let filename = file.name();

                                                // Read file content
                                                let array_buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                                                    .await
                                                    .unwrap();
                                                let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                                                let data = uint8_array.to_vec();

                                                match api::upload_document(&filename, &data).await {
                                                    Ok(_resp) => {
                                                        upload_status.set(Some(format!("✓ {}", filename)));
                                                        // Refresh document list
                                                        if let Ok(docs) = api::list_documents().await {
                                                            documents.set(docs.documents);
                                                        }
                                                    }
                                                    Err(e) => {
                                                        upload_status.set(Some(format!("✗ {}", e)));
                                                    }
                                                }
                                            }
                                        }

                                        is_uploading.set(false);
                                    });
                                },
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
                                            class: "text-xs truncate py-0.5 px-1 hover:bg-base-200 rounded",
                                            title: "{doc}",
                                            "📄 {doc}"
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
                class: "flex-1 flex flex-col min-w-0 h-full overflow-hidden",

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

                // Messages area - scrollable, takes remaining space
                div {
                    class: "flex-1 overflow-y-auto min-h-0 p-2 sm:p-4",

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
                    }
                }

            }

            // Input area - fixed at bottom, centered on viewport
            div {
                class: "fixed bottom-0 inset-x-0 p-2 sm:p-3",

                // Input box with Send button inside - centered relative to full viewport (aligns with header title)
                div {
                    class: "mx-auto mb-2",
                    style: "width: 40rem; max-width: 90vw; margin-left: calc(50% - 20rem + 1cm);",
                    
                    div {
                        class: "relative w-full",
                        
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
                                    li { "Upload documents (.txt, .md, .pdf)" }
                                    li { "Ask questions in the chat" }
                                    li { "Relevant content is found and sent to the LLM" }
                                    li { "Get answers grounded in your documents" }
                                }
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
        }
    }
}
