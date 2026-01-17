//! Streaming Response Utilities
//! 
//! Provides utilities for streaming large responses instead of buffering.
//! This reduces memory usage and improves time-to-first-byte.

use actix_web::{web, HttpResponse};
use futures_util::stream::{self, Stream};
use serde::Serialize;
use tokio::sync::mpsc;

/// Streaming JSON array response
/// 
/// Streams JSON array elements one at a time instead of buffering the entire array.
pub struct JsonArrayStream<T> {
    items: Vec<T>,
    index: usize,
    started: bool,
    finished: bool,
}

impl<T: Serialize> JsonArrayStream<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            index: 0,
            started: false,
            finished: false,
        }
    }

    /// Convert to a byte stream for HTTP response
    pub fn into_stream(self) -> impl Stream<Item = Result<web::Bytes, std::io::Error>> {
        stream::unfold(self, |mut state| async move {
            if state.finished {
                return None;
            }

            let bytes = if !state.started {
                state.started = true;
                web::Bytes::from_static(b"[")
            } else if state.index < state.items.len() {
                let item = &state.items[state.index];
                let json = serde_json::to_string(item).unwrap_or_default();
                let prefix = if state.index > 0 { "," } else { "" };
                state.index += 1;
                web::Bytes::from(format!("{}{}", prefix, json))
            } else {
                state.finished = true;
                web::Bytes::from_static(b"]")
            };

            Some((Ok(bytes), state))
        })
    }
}

/// Create a streaming HTTP response for a JSON array
pub fn streaming_json_array<T: Serialize + 'static>(items: Vec<T>) -> HttpResponse {
    let stream = JsonArrayStream::new(items).into_stream();
    HttpResponse::Ok()
        .content_type("application/json")
        .streaming(stream)
}

/// Streaming search results
#[derive(Serialize)]
pub struct StreamingSearchResult {
    pub doc_id: String,
    pub score: f32,
    pub content: String,
}

/// Channel-based streaming for async producers
pub struct ChannelStream<T> {
    receiver: mpsc::Receiver<T>,
}

impl<T> ChannelStream<T> {
    pub fn new(buffer_size: usize) -> (mpsc::Sender<T>, Self) {
        let (sender, receiver) = mpsc::channel(buffer_size);
        (sender, Self { receiver })
    }
}

impl<T: Serialize + Send + 'static> ChannelStream<T> {
    pub fn into_json_stream(self) -> impl Stream<Item = Result<web::Bytes, std::io::Error>> {
        stream::unfold((self.receiver, false, false), |(mut rx, started, finished)| async move {
            if finished {
                return None;
            }

            if !started {
                return Some((Ok(web::Bytes::from_static(b"[")), (rx, true, false)));
            }

            match rx.recv().await {
                Some(item) => {
                    let json = serde_json::to_string(&item).unwrap_or_default();
                    let bytes = web::Bytes::from(format!(",{}", json));
                    Some((Ok(bytes), (rx, true, false)))
                }
                None => {
                    Some((Ok(web::Bytes::from_static(b"]")), (rx, true, true)))
                }
            }
        })
    }
}

/// Newline-delimited JSON (NDJSON) stream
/// 
/// Each item is a separate JSON object followed by a newline.
/// Useful for streaming to clients that process line-by-line.
pub struct NdjsonStream<T> {
    items: Vec<T>,
    index: usize,
}

impl<T: Serialize> NdjsonStream<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items, index: 0 }
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<web::Bytes, std::io::Error>> {
        stream::unfold(self, |mut state| async move {
            if state.index >= state.items.len() {
                return None;
            }

            let item = &state.items[state.index];
            let json = serde_json::to_string(item).unwrap_or_default();
            state.index += 1;
            
            Some((Ok(web::Bytes::from(format!("{}\n", json))), state))
        })
    }
}

/// Create a streaming NDJSON response
pub fn streaming_ndjson<T: Serialize + 'static>(items: Vec<T>) -> HttpResponse {
    let stream = NdjsonStream::new(items).into_stream();
    HttpResponse::Ok()
        .content_type("application/x-ndjson")
        .streaming(stream)
}

/// Server-Sent Events (SSE) stream
pub struct SseStream<T> {
    items: Vec<T>,
    index: usize,
    event_type: String,
}

impl<T: Serialize> SseStream<T> {
    pub fn new(items: Vec<T>, event_type: &str) -> Self {
        Self {
            items,
            index: 0,
            event_type: event_type.to_string(),
        }
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<web::Bytes, std::io::Error>> {
        stream::unfold(self, |mut state| async move {
            if state.index >= state.items.len() {
                return None;
            }

            let item = &state.items[state.index];
            let json = serde_json::to_string(item).unwrap_or_default();
            state.index += 1;
            
            let sse = format!("event: {}\ndata: {}\n\n", state.event_type, json);
            Some((Ok(web::Bytes::from(sse)), state))
        })
    }
}

/// Create a streaming SSE response
pub fn streaming_sse<T: Serialize + 'static>(items: Vec<T>, event_type: &str) -> HttpResponse {
    let stream = SseStream::new(items, event_type).into_stream();
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(stream)
}

/// Chunked transfer encoding helper
pub fn chunked_response<S>(stream: S) -> HttpResponse
where
    S: Stream<Item = Result<web::Bytes, std::io::Error>> + 'static,
{
    HttpResponse::Ok()
        .insert_header(("Transfer-Encoding", "chunked"))
        .streaming(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[derive(Serialize, Clone)]
    struct TestItem {
        id: i32,
        name: String,
    }

    #[tokio::test]
    async fn test_json_array_stream() {
        let items = vec![
            TestItem { id: 1, name: "one".to_string() },
            TestItem { id: 2, name: "two".to_string() },
        ];

        let stream = JsonArrayStream::new(items).into_stream();
        let bytes: Vec<_> = stream.collect().await;
        
        let json: String = bytes
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|b| String::from_utf8(b.to_vec()).unwrap())
            .collect();
        
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"id\":2"));
    }

    #[tokio::test]
    async fn test_ndjson_stream() {
        let items = vec![
            TestItem { id: 1, name: "one".to_string() },
            TestItem { id: 2, name: "two".to_string() },
        ];

        let stream = NdjsonStream::new(items).into_stream();
        let bytes: Vec<_> = stream.collect().await;
        
        assert_eq!(bytes.len(), 2);
        
        let line1 = String::from_utf8(bytes[0].as_ref().unwrap().to_vec()).unwrap();
        assert!(line1.ends_with('\n'));
        assert!(line1.contains("\"id\":1"));
    }
}
