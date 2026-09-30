//! `internot_renderer` — shared infrastructure for LLM-driven
//! rendering of substrate AVMs.
//!
//! Mirrors the architecture in CLAUDE.md's "Stage Manager paradigm"
//! section: each service crate owns its own typed renderer +
//! per-service AVM JSON wrapper + system prompt; this crate provides
//! the cross-cutting plumbing (HTTP client, structured-output schema
//! wrapping, on-disk cache).
//!
//! Strategic motivation: counter to the Microsoft *Synthetic Computers*
//! paper's LLM-grounded approach. Internot's procedural floor + cached
//! LLM rendering should produce coherent multi-message artifacts at
//! significantly lower marginal cost — every render hits cache after
//! the first call for a given (id, prompt_version, model_version) tuple.

pub mod cache;
pub mod error;
pub mod openai;

pub use cache::{Cache, CacheKey};
pub use error::RenderError;
pub use openai::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, OpenAiClient,
    ResponseFormat,
};
