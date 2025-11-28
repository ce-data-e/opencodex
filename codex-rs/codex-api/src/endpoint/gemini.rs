//! Endpoint client for Google Gemini API.
//!
//! Handles requests to Gemini's generateContent endpoint.
//! Unlike OpenAI-style endpoints, Gemini uses a model-in-path URL pattern.
//! Note: Uses non-streaming requests as some gateways don't support streamGenerateContent.

use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::common::Prompt as ApiPrompt;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::GeminiRequest;
use crate::requests::GeminiRequestBuilder;
use crate::telemetry::SseTelemetry;
use crate::telemetry::run_with_request_telemetry;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::RequestTelemetry;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use http::HeaderMap;
use http::Method;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct GeminiClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
    #[allow(dead_code)]
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport, A: AuthProvider> GeminiClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
            request_telemetry: None,
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        mut self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        self.request_telemetry = request;
        self.sse_telemetry = sse;
        self
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    pub async fn stream_request(&self, request: GeminiRequest) -> Result<ResponseStream, ApiError> {
        self.stream(&request.model, request.body, request.headers)
            .await
    }

    pub async fn stream_prompt(
        &self,
        model: &str,
        prompt: &ApiPrompt,
        conversation_id: Option<String>,
        session_source: Option<SessionSource>,
    ) -> Result<ResponseStream, ApiError> {
        let request =
            GeminiRequestBuilder::new(model, &prompt.instructions, &prompt.input, &prompt.tools)
                .conversation_id(conversation_id)
                .session_source(session_source)
                .build(&self.provider)?;

        self.stream_request(request).await
    }

    pub async fn stream(
        &self,
        model: &str,
        body: Value,
        extra_headers: HeaderMap,
    ) -> Result<ResponseStream, ApiError> {
        let url = self.provider.gemini_url_for_model(model);

        let builder = || {
            let mut req = Request {
                method: Method::POST,
                url: url.clone(),
                headers: self.provider.headers.clone(),
                body: Some(body.clone()),
                timeout: None,
            };
            req.headers.extend(extra_headers.clone());
            req.headers.insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("application/json"),
            );
            add_auth_headers(&self.auth, req)
        };

        // Use non-streaming request since many gateways don't support streamGenerateContent
        let response = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            builder,
            |req| self.transport.execute(req),
        )
        .await?;

        // Parse the response and convert to events
        let gemini_response: GeminiResponse = serde_json::from_slice(&response.body)
            .map_err(|e| ApiError::Stream(format!("Failed to parse Gemini response: {}", e)))?;

        // Create channel for events
        let (tx, rx_event) = mpsc::channel(32);

        // Spawn task to emit events
        tokio::spawn(async move {
            emit_gemini_events(tx, gemini_response).await;
        });

        Ok(ResponseStream { rx_event })
    }
}

/// Gemini API response structures for non-streaming
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    usage_metadata: Option<GeminiUsageMetadata>,
    #[allow(dead_code)]
    model_version: Option<String>,
    #[allow(dead_code)]
    response_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
struct GeminiFunctionCall {
    name: String,
    args: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    prompt_token_count: Option<i32>,
    candidates_token_count: Option<i32>,
    total_token_count: Option<i32>,
}

async fn emit_gemini_events(
    tx: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    response: GeminiResponse,
) {
    let mut function_call_counter = 0;

    if let Some(candidates) = response.candidates {
        for candidate in candidates {
            if let Some(content) = candidate.content {
                if let Some(parts) = content.parts {
                    let mut text_parts = Vec::new();

                    for part in parts {
                        // Collect text parts
                        if let Some(text) = part.text {
                            if !text.is_empty() {
                                text_parts.push(text.clone());
                                // Emit text delta
                                let _ = tx
                                    .send(Ok(ResponseEvent::OutputTextDelta(text)))
                                    .await;
                            }
                        }

                        // Handle function calls
                        if let Some(func_call) = part.function_call {
                            function_call_counter += 1;
                            let call_id = format!("gemini_call_{}", function_call_counter);

                            let arguments = func_call
                                .args
                                .map(|a| serde_json::to_string(&a).unwrap_or_else(|_| "{}".to_string()))
                                .unwrap_or_else(|| "{}".to_string());

                            let item = ResponseItem::FunctionCall {
                                id: None,
                                name: func_call.name,
                                arguments,
                                call_id,
                            };
                            let _ = tx.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                        }
                    }

                    // Emit message if we had text
                    if !text_parts.is_empty() {
                        let full_text = text_parts.join("");
                        let message = ResponseItem::Message {
                            id: None,
                            role: "assistant".to_string(),
                            content: vec![ContentItem::OutputText { text: full_text }],
                        };
                        let _ = tx.send(Ok(ResponseEvent::OutputItemDone(message))).await;
                    }
                }
            }

            // Check finish reason for errors
            if let Some(reason) = &candidate.finish_reason {
                if reason == "MAX_TOKENS" {
                    let _ = tx.send(Err(ApiError::ContextWindowExceeded)).await;
                    return;
                } else if reason == "SAFETY" {
                    let _ = tx
                        .send(Err(ApiError::Stream(
                            "Response blocked by safety filters".to_string(),
                        )))
                        .await;
                    return;
                }
            }
        }
    }

    // Emit completed event with token usage
    let token_usage = response.usage_metadata.map(|u| TokenUsage {
        input_tokens: i64::from(u.prompt_token_count.unwrap_or(0)),
        output_tokens: i64::from(u.candidates_token_count.unwrap_or(0)),
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
        total_tokens: i64::from(u.total_token_count.unwrap_or(0)),
    });

    let _ = tx
        .send(Ok(ResponseEvent::Completed {
            response_id: String::new(),
            token_usage,
        }))
        .await;
}

#[cfg(test)]
mod tests {
    use crate::provider::Provider;
    use crate::provider::RetryConfig;
    use crate::provider::WireApi;
    use http::HeaderMap;
    use std::time::Duration;

    fn provider() -> Provider {
        Provider {
            name: "gemini".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            query_params: None,
            wire: WireApi::Gemini,
            model_name: Some("gemini-pro".to_string()),
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(10),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn constructs_gemini_url() {
        let p = provider();
        let url = p.gemini_url_for_model("gemini-pro");
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn constructs_gemini_url_with_custom_base() {
        let mut p = provider();
        p.base_url = "https://ai-gateway.example.com/google/v1beta1/publishers/google".to_string();
        let url = p.gemini_url_for_model("gemini-3-pro-preview");
        assert_eq!(
            url,
            "https://ai-gateway.example.com/google/v1beta1/publishers/google/models/gemini-3-pro-preview:generateContent"
        );
    }
}
