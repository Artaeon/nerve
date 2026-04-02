use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, anyhow};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::provider::{AiProvider, ChatMessage, ModelInfo, StreamEvent};

// ---------------------------------------------------------------------------
// Request / response types for the OpenAI-compatible API
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

// -- Non-streaming response --

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: Option<String>,
}

// -- Streaming SSE chunk --

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

// -- Models listing --

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    context_length: Option<usize>,
}

// -- API error body --

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: Option<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

/// An AI provider that speaks the OpenAI chat-completions protocol.
///
/// Works with OpenAI, Ollama, OpenRouter, and any compatible endpoint.
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    provider_name: String,
    max_tokens: Option<usize>,
}

impl OpenAiProvider {
    /// Create a new provider with explicit configuration.
    pub fn new(api_key: String, base_url: String, provider_name: String) -> Self {
        let client = reqwest::Client::new();
        Self {
            client,
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            provider_name,
            max_tokens: Some(4096),
        }
    }

    /// Override the `max_tokens` sent with each request.
    #[allow(dead_code)]
    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    // -- Convenience constructors --------------------------------------------------

    /// OpenAI (api.openai.com).
    #[allow(dead_code)]
    pub fn openai(api_key: String) -> Self {
        Self::new(api_key, "https://api.openai.com/v1".into(), "OpenAI".into())
    }

    /// Ollama running locally on the default port.
    #[allow(dead_code)]
    pub fn ollama() -> Self {
        Self::new(
            "ollama".into(),
            "http://localhost:11434/v1".into(),
            "Ollama".into(),
        )
    }

    /// OpenRouter.
    #[allow(dead_code)]
    pub fn openrouter(api_key: String) -> Self {
        Self::new(
            api_key,
            "https://openrouter.ai/api/v1".into(),
            "OpenRouter".into(),
        )
    }

    /// Any custom OpenAI-compatible endpoint.
    #[allow(dead_code)]
    pub fn custom(api_key: String, base_url: String, name: String) -> Self {
        Self::new(api_key, base_url, name)
    }

    // -- Internal helpers ----------------------------------------------------------

    /// Build a request with the common headers.
    fn auth_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
    }

    /// Try to extract an error message from a non-2xx response body.
    async fn extract_api_error(response: reqwest::Response) -> String {
        let status = response.status();
        match response.text().await {
            Ok(body) => {
                if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body)
                    && let Some(detail) = err.error
                    && let Some(msg) = detail.message
                {
                    return format!("API error ({status}): {msg}");
                }
                format!("API error ({status}): {body}")
            }
            Err(_) => format!("API error ({status})"),
        }
    }
}

impl AiProvider for OpenAiProvider {
    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let messages = messages.to_vec();
        let model = model.to_string();
        Box::pin(async move {
            let url = format!("{}/chat/completions", self.base_url);
            let body = ChatCompletionRequest {
                model: &model,
                messages: &messages,
                stream: true,
                max_tokens: self.max_tokens,
            };

            let response = self
                .auth_request(reqwest::Method::POST, &url)
                .json(&body)
                .send()
                .await
                .context("failed to send streaming request")?;

            if !response.status().is_success() {
                let msg = Self::extract_api_error(response).await;
                let _ = tx.send(StreamEvent::Error(msg.clone()));
                return Err(anyhow!(msg));
            }

            // Read the byte stream and parse SSE lines.
            let mut byte_stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        let msg = format!("stream read error: {e}");
                        let _ = tx.send(StreamEvent::Error(msg.clone()));
                        return Err(anyhow!(msg));
                    }
                };

                // Append raw bytes to buffer (UTF-8).
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                // Process complete lines from the buffer.
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    // Skip empty lines and SSE comments (lines starting with ':')
                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    // We only care about "data: ..." lines.
                    let Some(data) = line.strip_prefix("data: ") else {
                        // Could be "event:" or other SSE field — ignore.
                        continue;
                    };

                    let data = data.trim();

                    // End-of-stream sentinel.
                    if data == "[DONE]" {
                        let _ = tx.send(StreamEvent::Done);
                        return Ok(());
                    }

                    // Skip empty data payloads.
                    if data.is_empty() {
                        continue;
                    }

                    // Parse the JSON chunk.
                    match serde_json::from_str::<ChatCompletionChunk>(data) {
                        Ok(chunk) => {
                            if let Some(choice) = chunk.choices.first()
                                && let Some(ref content) = choice.delta.content
                                && !content.is_empty()
                                && tx.send(StreamEvent::Token(content.clone())).is_err()
                            {
                                // Receiver dropped — stop processing.
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            // Log but don't abort — some providers send non-standard
                            // chunks (e.g. usage data) that we can safely ignore.
                            tracing::warn!("failed to parse SSE chunk: {e} — data: {data}");
                        }
                    }
                }
            }

            // Stream ended without an explicit [DONE] — still signal completion.
            let _ = tx.send(StreamEvent::Done);
            Ok(())
        })
    }

    fn chat(
        &self,
        messages: &[ChatMessage],
        model: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        let messages = messages.to_vec();
        let model = model.to_string();
        Box::pin(async move {
            let url = format!("{}/chat/completions", self.base_url);
            let body = ChatCompletionRequest {
                model: &model,
                messages: &messages,
                stream: false,
                max_tokens: self.max_tokens,
            };

            let response = self
                .auth_request(reqwest::Method::POST, &url)
                .json(&body)
                .send()
                .await
                .context("failed to send chat request")?;

            if !response.status().is_success() {
                let msg = Self::extract_api_error(response).await;
                return Err(anyhow!(msg));
            }

            let resp: ChatCompletionResponse = response
                .json()
                .await
                .context("failed to parse chat completion response")?;

            let content = resp
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .unwrap_or_default();

            Ok(content)
        })
    }

    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>> {
        Box::pin(async move {
            let url = format!("{}/models", self.base_url);

            let response = self
                .auth_request(reqwest::Method::GET, &url)
                .send()
                .await
                .context("failed to fetch models list")?;

            if !response.status().is_success() {
                let msg = Self::extract_api_error(response).await;
                return Err(anyhow!(msg));
            }

            let resp: ModelsResponse = response
                .json()
                .await
                .context("failed to parse models response")?;

            let models = resp
                .data
                .into_iter()
                .map(|entry| {
                    let display_name = entry.name.unwrap_or_else(|| entry.id.clone());
                    ModelInfo {
                        id: entry.id,
                        name: display_name,
                        provider: self.provider_name.clone(),
                        context_length: entry.context_length,
                    }
                })
                .collect();

            Ok(models)
        })
    }

    fn name(&self) -> &str {
        &self.provider_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_chat_response() {
        let json = r#"{"choices":[{"message":{"content":"Hello!"}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content.as_deref(), Some("Hello!"));
    }

    #[test]
    fn deserialize_chat_chunk() {
        let json = r#"{"choices":[{"delta":{"content":"Hi"}}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("Hi"));
    }

    #[test]
    fn deserialize_chunk_with_null_content() {
        let json = r#"{"choices":[{"delta":{}}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices[0].delta.content.is_none());
    }

    #[test]
    fn deserialize_models_response() {
        let json = r#"{"data":[{"id":"gpt-4o","name":"GPT-4o","context_length":128000}]}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "gpt-4o");
    }

    #[test]
    fn deserialize_error_response() {
        let json = r#"{"error":{"message":"Invalid API key"}}"#;
        let resp: ApiErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.error.unwrap().message.unwrap(), "Invalid API key");
    }

    #[test]
    fn empty_choices_handled() {
        let json = r#"{"choices":[]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert!(resp.choices.is_empty());
    }

    #[test]
    fn deserialize_chunk_empty_choices() {
        let json = r#"{"choices":[]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices.is_empty());
    }

    #[test]
    fn deserialize_model_entry_without_optional_fields() {
        let json = r#"{"data":[{"id":"llama3"}]}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data[0].id, "llama3");
        assert!(resp.data[0].name.is_none());
        assert!(resp.data[0].context_length.is_none());
    }

    #[test]
    fn deserialize_error_response_with_null_error() {
        let json = r#"{"error":null}"#;
        let resp: ApiErrorResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_none());
    }

    #[test]
    fn deserialize_chat_response_null_content() {
        let json = r#"{"choices":[{"message":{}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert!(resp.choices[0].message.content.is_none());
    }

    #[test]
    fn deserialize_multiple_choices() {
        let json = r#"{"choices":[{"message":{"content":"A"}},{"message":{"content":"B"}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 2);
        assert_eq!(resp.choices[0].message.content.as_deref(), Some("A"));
        assert_eq!(resp.choices[1].message.content.as_deref(), Some("B"));
    }

    #[test]
    fn deserialize_multiple_model_entries() {
        let json = r#"{"data":[{"id":"gpt-4o"},{"id":"gpt-4o-mini","name":"GPT-4o Mini","context_length":16384}]}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[1].id, "gpt-4o-mini");
        assert_eq!(resp.data[1].context_length, Some(16384));
    }

    // ── Security: malformed / adversarial JSON responses ───────────────

    #[test]
    fn deserialize_malformed_json_missing_choices() {
        let json = r#"{"not_a_valid_response": true}"#;
        let result = serde_json::from_str::<ChatCompletionResponse>(json);
        assert!(
            result.is_err(),
            "Missing required 'choices' field should fail"
        );
    }

    #[test]
    fn deserialize_empty_object() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<ChatCompletionResponse>(json);
        assert!(result.is_err(), "Empty object should fail deserialization");
    }

    #[test]
    fn deserialize_very_long_content() {
        let long_content = "x".repeat(1_000_000);
        let json = format!(r#"{{"choices":[{{"message":{{"content":"{long_content}"}}}}]}}"#);
        let resp: ChatCompletionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(
            resp.choices[0].message.content.as_ref().unwrap().len(),
            1_000_000
        );
    }

    #[test]
    fn deserialize_chunk_malformed_json() {
        let json = r#"{"not_valid": 42}"#;
        let result = serde_json::from_str::<ChatCompletionChunk>(json);
        assert!(result.is_err(), "Missing 'choices' should fail for chunk");
    }

    #[test]
    fn deserialize_response_with_extra_fields_ignored() {
        // Serde should ignore unknown fields by default
        let json = r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"total_tokens":42},"id":"chatcmpl-xyz"}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices[0].message.content.as_deref(), Some("ok"));
    }

    #[test]
    fn deserialize_chunk_with_extra_fields() {
        let json = r#"{"choices":[{"delta":{"content":"hi"},"index":0,"finish_reason":null}],"id":"chunk-1"}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("hi"));
    }

    #[test]
    fn deserialize_error_response_empty_object() {
        let json = r#"{}"#;
        let resp: ApiErrorResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_none());
    }

    #[test]
    fn deserialize_choices_wrong_type() {
        let json = r#"{"choices":"not_an_array"}"#;
        let result = serde_json::from_str::<ChatCompletionResponse>(json);
        assert!(result.is_err(), "choices as string should fail");
    }

    #[test]
    fn deserialize_content_with_special_json_chars() {
        let json = r#"{"choices":[{"message":{"content":"line1\nline2\t\"quoted\""}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let content = resp.choices[0].message.content.as_ref().unwrap();
        assert!(content.contains("line1\nline2"));
        assert!(content.contains("\"quoted\""));
    }

    #[test]
    fn deserialize_content_with_unicode() {
        let json = r#"{"choices":[{"message":{"content":"Hello \u4e16\u754c \ud83c\udf0d"}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let content = resp.choices[0].message.content.as_ref().unwrap();
        assert!(content.contains('\u{4e16}')); // Chinese character
    }
}
