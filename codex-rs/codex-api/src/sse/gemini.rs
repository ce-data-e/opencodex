//! SSE parser for Google Gemini streaming responses.
//!
//! Parses Gemini's `streamGenerateContent` SSE events and converts them
//! to Codex ResponseEvent format.

use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use codex_client::StreamResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::StreamExt;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

#[allow(dead_code)]
pub(crate) fn spawn_gemini_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<std::sync::Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_gemini_sse(stream_response.bytes, tx_event, idle_timeout, telemetry).await;
    });
    ResponseStream { rx_event }
}

/// Gemini SSE response structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[allow(dead_code)]
    role: Option<String>,
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPart {
    text: Option<String>,
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: Option<serde_json::Value>,
    /// Thought signature for Gemini thinking mode
    #[serde(rename = "thoughtSignature")]
    thought_signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    prompt_token_count: Option<i64>,
    candidates_token_count: Option<i64>,
    total_token_count: Option<i64>,
}

pub async fn process_gemini_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<std::sync::Arc<dyn SseTelemetry>>,
) where
    S: Stream<Item = Result<bytes::Bytes, codex_client::TransportError>> + Unpin,
{
    let mut stream = stream.eventsource();

    let mut assistant_item: Option<ResponseItem> = None;
    let mut completed_sent = false;
    let mut last_usage: Option<GeminiUsageMetadata> = None;
    let mut function_call_counter: u64 = 0;

    loop {
        let start = Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        if let Some(t) = telemetry.as_ref() {
            t.on_sse_poll(&response, start.elapsed());
        }

        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                let _ = tx_event.send(Err(ApiError::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                // Stream ended
                if let Some(assistant) = assistant_item.take() {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                        .await;
                }
                if !completed_sent {
                    let token_usage = last_usage.map(|u| TokenUsage {
                        input_tokens: u.prompt_token_count.unwrap_or(0),
                        output_tokens: u.candidates_token_count.unwrap_or(0),
                        cached_input_tokens: 0,
                        reasoning_output_tokens: 0,
                        total_tokens: u.total_token_count.unwrap_or(0),
                    });
                    let _ = tx_event
                        .send(Ok(ResponseEvent::Completed {
                            response_id: String::new(),
                            token_usage,
                        }))
                        .await;
                }
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        trace!("Gemini SSE event: {}", sse.data);

        if sse.data.trim().is_empty() {
            continue;
        }

        let gemini_response: GeminiResponse = match serde_json::from_str(&sse.data) {
            Ok(val) => val,
            Err(err) => {
                debug!(
                    "Failed to parse Gemini SSE event: {err}, data: {}",
                    &sse.data
                );
                continue;
            }
        };

        // Store usage metadata for later
        if let Some(usage) = gemini_response.usage_metadata {
            last_usage = Some(usage);
        }

        // Process candidates
        let Some(candidates) = gemini_response.candidates else {
            continue;
        };

        for candidate in candidates {
            // Process content parts FIRST (before checking finish reason)
            if let Some(content) = &candidate.content
                && let Some(parts) = &content.parts
            {
                for part in parts {
                    // Handle text parts
                    if let Some(text) = &part.text
                        && !text.is_empty()
                    {
                        append_assistant_text(&tx_event, &mut assistant_item, text.clone()).await;
                    }

                    // Handle function calls
                    if let Some(func_call) = &part.function_call {
                        // First, emit any pending assistant message
                        if let Some(assistant) = assistant_item.take() {
                            let _ = tx_event
                                .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                                .await;
                        }

                        // Generate a unique call_id
                        function_call_counter += 1;
                        let call_id = format!("gemini_call_{function_call_counter}");

                        // Serialize args to string
                        let arguments = func_call
                            .args
                            .as_ref()
                            .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "{}".to_string()))
                            .unwrap_or_else(|| "{}".to_string());

                        let item = ResponseItem::FunctionCall {
                            id: None,
                            name: func_call.name.clone(),
                            arguments,
                            call_id,
                            thought_signature: func_call.thought_signature.clone(),
                        };
                        let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                    }
                }
            }

            // Handle finish reason AFTER content processing
            if let Some(reason) = &candidate.finish_reason {
                match reason.as_str() {
                    "STOP" => {
                        // Normal completion
                        if let Some(assistant) = assistant_item.take() {
                            let _ = tx_event
                                .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                                .await;
                        }
                        if !completed_sent {
                            let token_usage = last_usage.take().map(|u| TokenUsage {
                                input_tokens: u.prompt_token_count.unwrap_or(0),
                                output_tokens: u.candidates_token_count.unwrap_or(0),
                                cached_input_tokens: 0,
                                reasoning_output_tokens: 0,
                                total_tokens: u.total_token_count.unwrap_or(0),
                            });
                            let _ = tx_event
                                .send(Ok(ResponseEvent::Completed {
                                    response_id: String::new(),
                                    token_usage,
                                }))
                                .await;
                            completed_sent = true;
                        }
                    }
                    "MAX_TOKENS" => {
                        let _ = tx_event.send(Err(ApiError::ContextWindowExceeded)).await;
                        return;
                    }
                    "SAFETY" => {
                        let _ = tx_event
                            .send(Err(ApiError::Stream(
                                "Response blocked by safety filters".to_string(),
                            )))
                            .await;
                        return;
                    }
                    _ => {
                        // Other finish reasons - just log and continue
                        debug!("Gemini finish reason: {}", reason);
                    }
                }
            }
        }
    }
}

async fn append_assistant_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    assistant_item: &mut Option<ResponseItem>,
    text: String,
) {
    if assistant_item.is_none() {
        let item = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![],
        };
        *assistant_item = Some(item.clone());
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemAdded(item)))
            .await;
    }

    if let Some(ResponseItem::Message { content, .. }) = assistant_item {
        content.push(ContentItem::OutputText { text: text.clone() });
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputTextDelta(text)))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use futures::TryStreamExt;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::io::ReaderStream;

    fn build_body(events: &[serde_json::Value]) -> String {
        let mut body = String::new();
        for e in events {
            body.push_str(&format!("data: {e}\n\n"));
        }
        body
    }

    async fn collect_events(body: &str) -> Vec<ResponseEvent> {
        let reader = ReaderStream::new(std::io::Cursor::new(body.to_string()))
            .map_err(|err| codex_client::TransportError::Network(err.to_string()));
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent, ApiError>>(16);
        tokio::spawn(process_gemini_sse(
            reader,
            tx,
            Duration::from_millis(1000),
            None,
        ));

        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            out.push(ev.expect("stream error"));
        }
        out
    }

    #[tokio::test]
    async fn parses_text_response() {
        let chunk1 = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello, "}]
                }
            }]
        });

        let chunk2 = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "world!"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        });

        let body = build_body(&[chunk1, chunk2]);
        let events = collect_events(&body).await;

        assert_matches!(
            &events[0],
            ResponseEvent::OutputItemAdded(ResponseItem::Message { role, .. })
            if role == "assistant"
        );
        assert_matches!(&events[1], ResponseEvent::OutputTextDelta(t) if t == "Hello, ");
        assert_matches!(&events[2], ResponseEvent::OutputTextDelta(t) if t == "world!");
        assert_matches!(
            &events[3],
            ResponseEvent::OutputItemDone(ResponseItem::Message { .. })
        );
        assert_matches!(
            &events[4],
            ResponseEvent::Completed { token_usage, .. }
            if token_usage.as_ref().map(|u| u.input_tokens) == Some(10)
        );
    }

    #[tokio::test]
    async fn parses_function_call() {
        let chunk = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "functionCall": {
                            "name": "get_weather",
                            "args": {"city": "London"}
                        }
                    }]
                },
                "finishReason": "STOP"
            }]
        });

        let body = build_body(&[chunk]);
        let events = collect_events(&body).await;

        assert_matches!(
            &events[0],
            ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { name, arguments, .. })
            if name == "get_weather" && arguments.contains("London")
        );
        assert_matches!(&events[1], ResponseEvent::Completed { .. });
    }

    #[tokio::test]
    async fn handles_max_tokens_finish_reason() {
        let chunk = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "partial"}]
                },
                "finishReason": "MAX_TOKENS"
            }]
        });

        let body = build_body(&[chunk]);
        let reader = ReaderStream::new(std::io::Cursor::new(body))
            .map_err(|err| codex_client::TransportError::Network(err.to_string()));
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent, ApiError>>(16);
        tokio::spawn(process_gemini_sse(
            reader,
            tx,
            Duration::from_millis(1000),
            None,
        ));

        // Collect all results including errors
        let mut found_error = false;
        while let Some(ev) = rx.recv().await {
            if let Err(ApiError::ContextWindowExceeded) = ev {
                found_error = true;
            }
        }
        assert!(found_error, "expected ContextWindowExceeded error");
    }
}
