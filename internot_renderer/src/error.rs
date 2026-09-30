use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("missing OPENAI_API_KEY env var")]
    MissingApiKey,

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("OpenAI returned no choices")]
    EmptyChoices,

    #[error("OpenAI returned a choice with no content")]
    EmptyContent,

    #[error("could not parse model output as expected JSON: {0}")]
    Parse(String),

    #[error("cache I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON encode error: {0}")]
    Encode(#[source] serde_json::Error),

    #[error("JSON decode error: {0}")]
    Decode(#[source] serde_json::Error),
}
