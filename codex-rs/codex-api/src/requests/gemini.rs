//! Request builder for Google Gemini API.
//!
//! Converts Codex internal message format (ResponseItem) to Gemini's
//! `contents[].parts[]` format.

use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::headers::build_conversation_headers;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use serde_json::Value;
use serde_json::json;

/// Assembled request body plus headers for Gemini streaming calls.
pub struct GeminiRequest {
    pub body: Value,
    pub headers: HeaderMap,
    /// Model name to use in the URL path (e.g., "gemini-3-pro-preview").
    pub model: String,
}

pub struct GeminiRequestBuilder<'a> {
    model: &'a str,
    instructions: &'a str,
    input: &'a [ResponseItem],
    tools: &'a [Value],
    conversation_id: Option<String>,
    #[allow(dead_code)]
    session_source: Option<SessionSource>,
}

impl<'a> GeminiRequestBuilder<'a> {
    pub fn new(
        model: &'a str,
        instructions: &'a str,
        input: &'a [ResponseItem],
        tools: &'a [Value],
    ) -> Self {
        Self {
            model,
            instructions,
            input,
            tools,
            conversation_id: None,
            session_source: None,
        }
    }

    pub fn conversation_id(mut self, id: Option<String>) -> Self {
        self.conversation_id = id;
        self
    }

    pub fn session_source(mut self, source: Option<SessionSource>) -> Self {
        self.session_source = source;
        self
    }

    pub fn build(self, _provider: &Provider) -> Result<GeminiRequest, ApiError> {
        let mut contents = Vec::<Value>::new();

        // Track function call names for function responses
        let mut function_call_names: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        // First pass: collect function call names by call_id
        for item in self.input {
            if let ResponseItem::FunctionCall {
                name, call_id, ..
            } = item
            {
                function_call_names.insert(call_id.clone(), name.clone());
            }
        }

        for item in self.input {
            match item {
                ResponseItem::Message { role, content, .. } => {
                    let gemini_role = map_role_to_gemini(role);
                    let parts = content_items_to_parts(content);

                    if !parts.is_empty() {
                        contents.push(json!({
                            "role": gemini_role,
                            "parts": parts
                        }));
                    }
                }
                ResponseItem::FunctionCall {
                    name, arguments, ..
                } => {
                    // Parse arguments as JSON
                    let args: Value = serde_json::from_str(arguments).unwrap_or(json!({}));

                    contents.push(json!({
                        "role": "model",
                        "parts": [{
                            "functionCall": {
                                "name": name,
                                "args": args
                            }
                        }]
                    }));
                }
                ResponseItem::FunctionCallOutput { call_id, output } => {
                    // Look up the function name from the call_id
                    let function_name = function_call_names
                        .get(call_id)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());

                    // Build the response content
                    let response_value = if let Some(items) = &output.content_items {
                        // Multi-part response with images
                        let parts: Vec<Value> = items
                            .iter()
                            .filter_map(|it| match it {
                                FunctionCallOutputContentItem::InputText { text } => {
                                    Some(json!({ "text": text }))
                                }
                                FunctionCallOutputContentItem::InputImage { .. } => {
                                    // Gemini function responses don't support images directly
                                    None
                                }
                            })
                            .collect();
                        json!({ "parts": parts })
                    } else {
                        // Simple text response
                        json!({ "output": &output.content })
                    };

                    contents.push(json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": function_name,
                                "response": response_value
                            }
                        }]
                    }));
                }
                ResponseItem::LocalShellCall { .. } => {
                    // Local shell calls are handled as function calls
                    // Skip for now - they should be converted to FunctionCall first
                    continue;
                }
                ResponseItem::CustomToolCall { .. } | ResponseItem::CustomToolCallOutput { .. } => {
                    // Custom tools are not directly supported in Gemini
                    continue;
                }
                ResponseItem::Reasoning { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::GhostSnapshot { .. }
                | ResponseItem::CompactionSummary { .. }
                | ResponseItem::Other => {
                    continue;
                }
            }
        }

        // Build the request body
        let mut body = json!({
            "contents": contents
        });

        // Add system instruction if provided
        if !self.instructions.is_empty() {
            body["systemInstruction"] = json!({
                "parts": [{
                    "text": self.instructions
                }]
            });
        }

        // Add tools if provided (convert to Gemini function declarations format)
        if !self.tools.is_empty() {
            let function_declarations: Vec<Value> = self
                .tools
                .iter()
                .filter_map(|tool| {
                    // Convert from OpenAI tool format to Gemini format
                    if let Some(func) = tool.get("function") {
                        Some(json!({
                            "name": func.get("name"),
                            "description": func.get("description"),
                            "parameters": func.get("parameters")
                        }))
                    } else {
                        None
                    }
                })
                .collect();

            if !function_declarations.is_empty() {
                body["tools"] = json!([{
                    "functionDeclarations": function_declarations
                }]);
            }
        }

        let headers = build_conversation_headers(self.conversation_id);

        Ok(GeminiRequest {
            body,
            headers,
            model: self.model.to_string(),
        })
    }
}

/// Maps Codex/OpenAI role to Gemini role.
fn map_role_to_gemini(role: &str) -> &'static str {
    match role {
        "user" => "user",
        "assistant" => "model",
        "system" => "user", // System messages go to systemInstruction, not contents
        "tool" => "user",   // Tool responses are user messages with functionResponse
        _ => "user",
    }
}

/// Converts Codex ContentItem to Gemini parts format.
fn content_items_to_parts(content: &[ContentItem]) -> Vec<Value> {
    content
        .iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                if text.is_empty() {
                    None
                } else {
                    Some(json!({ "text": text }))
                }
            }
            ContentItem::InputImage { image_url } => {
                // Handle base64 data URLs or remote URLs
                if image_url.starts_with("data:") {
                    // Parse data URL: data:image/png;base64,<data>
                    if let Some(comma_idx) = image_url.find(',') {
                        let metadata = &image_url[5..comma_idx]; // Skip "data:"
                        let data = &image_url[comma_idx + 1..];

                        let mime_type = metadata
                            .split(';')
                            .next()
                            .unwrap_or("image/png");

                        Some(json!({
                            "inlineData": {
                                "mimeType": mime_type,
                                "data": data
                            }
                        }))
                    } else {
                        None
                    }
                } else {
                    // Remote URL - use fileData
                    Some(json!({
                        "fileData": {
                            "fileUri": image_url,
                            "mimeType": "image/jpeg"
                        }
                    }))
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RetryConfig;
    use crate::provider::WireApi;
    use pretty_assertions::assert_eq;
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
            stream_idle_timeout: Duration::from_secs(1),
        }
    }

    #[test]
    fn builds_simple_user_message() {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hello".to_string(),
            }],
        }];

        let request = GeminiRequestBuilder::new("gemini-pro", "You are helpful.", &input, &[])
            .build(&provider())
            .expect("request");

        let contents = request.body.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Hello");
    }

    #[test]
    fn includes_system_instruction() {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hi".to_string(),
            }],
        }];

        let request =
            GeminiRequestBuilder::new("gemini-pro", "Be concise.", &input, &[])
                .build(&provider())
                .expect("request");

        let system = request.body.get("systemInstruction").unwrap();
        assert_eq!(system["parts"][0]["text"], "Be concise.");
    }

    #[test]
    fn maps_assistant_to_model_role() {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "I'm an AI.".to_string(),
            }],
        }];

        let request = GeminiRequestBuilder::new("gemini-pro", "", &input, &[])
            .build(&provider())
            .expect("request");

        let contents = request.body.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents[0]["role"], "model");
    }

    #[test]
    fn converts_function_call() {
        let input = vec![ResponseItem::FunctionCall {
            id: None,
            name: "get_weather".to_string(),
            arguments: r#"{"city": "London"}"#.to_string(),
            call_id: "call_123".to_string(),
        }];

        let request = GeminiRequestBuilder::new("gemini-pro", "", &input, &[])
            .build(&provider())
            .expect("request");

        let contents = request.body.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents[0]["role"], "model");
        assert_eq!(contents[0]["parts"][0]["functionCall"]["name"], "get_weather");
        assert_eq!(contents[0]["parts"][0]["functionCall"]["args"]["city"], "London");
    }
}
