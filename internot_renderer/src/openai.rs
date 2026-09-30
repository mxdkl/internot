//! Minimal blocking-mode OpenAI Chat Completions client. Avoids
//! pulling in tokio for callers that don't need async — the renderer
//! is naturally request-per-message and the cache amortizes anyway.

use serde::{Deserialize, Serialize};

use crate::error::RenderError;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Clone)]
pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    client: reqwest::blocking::Client,
}

impl OpenAiClient {
    pub fn from_env() -> Result<Self, RenderError> {
        let api_key =
            std::env::var("OPENAI_API_KEY").map_err(|_| RenderError::MissingApiKey)?;
        Ok(Self::with_key(api_key))
    }

    pub fn with_key(api_key: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("blocking client builds");
        Self {
            api_key,
            base_url: DEFAULT_BASE_URL.to_string(),
            client,
        }
    }

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    pub fn chat_completion(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, RenderError> {
        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(req)
            .send()?
            .error_for_status()?;
        let body: ChatCompletionResponse = resp.json()?;
        Ok(body)
    }

    /// Convenience: make a single-message chat completion and return
    /// the first choice's content text. Errors if the response has
    /// zero choices or empty content.
    pub fn chat_completion_text(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<String, RenderError> {
        let resp = self.chat_completion(req)?;
        let choice = resp.choices.into_iter().next().ok_or(RenderError::EmptyChoices)?;
        choice.message.content.ok_or(RenderError::EmptyContent)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

impl ResponseFormat {
    /// Plain JSON object response (no schema enforcement).
    pub fn json_object() -> Self {
        Self { kind: "json_object".into(), json_schema: None }
    }

    /// Strict structured-output mode with a JSON schema.
    pub fn json_schema(name: &str, schema: serde_json::Value) -> Self {
        Self {
            kind: "json_schema".into(),
            json_schema: Some(serde_json::json!({
                "name": name,
                "strict": true,
                "schema": schema,
            })),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
