//! `View` — the canonical contract every operation goes through.
//!
//! A view is a typed function from `(Universe, Params) -> Output`. It
//! has a name, a description, an input shape (auto-derived JSON schema),
//! an output shape, and a static `READ_ONLY` flag.
//!
//! Transports never know which view they're running. They walk a
//! `ViewRegistry`, look up a view by name, deserialize the caller's
//! params, invoke the view, and serialize the output. The view itself
//! is transport-agnostic.
//!
//! # Layering
//!
//! - `View<P, O>` is the typed contract that service authors implement.
//! - `DynView` is the type-erased object-safe version transports use.
//! - A blanket `impl<V: View> DynView for V` makes every typed view
//!   automatically usable as `Arc<dyn DynView>`.
//! - `ViewRegistry` is the flat list, aggregated from each service's
//!   `views()` function.
//!
//! Each service exports `pub fn views() -> Vec<Arc<dyn DynView>>`
//! returning every view it owns.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::universe::Universe;

#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// Typed view contract. Service authors implement this on a unit
/// struct (or a small struct holding view-local config).
pub trait View: Send + Sync + 'static {
    type Params: JsonSchema + DeserializeOwned;
    type Output: Serialize;

    /// Tool/route name. Convention: `verb_entity` (`list_drive`,
    /// `read_file`, `rsvp`, `reply`). Names must be unique across the
    /// whole registry.
    const NAME: &'static str;

    /// Human-readable description. Surfaced as the MCP tool description
    /// and in agent prompts.
    const DESCRIPTION: &'static str;

    /// Whether this view mutates Universe state. Transports use this
    /// to route `GET` vs `POST`, and agents can use it to decide
    /// what's safe to call speculatively.
    const READ_ONLY: bool = true;

    fn execute(&self, ctx: &Universe, params: Self::Params) -> Result<Self::Output, ViewError>;
}

/// Type-erased object-safe view. Transports hold `Arc<dyn DynView>`s
/// in a flat registry.
pub trait DynView: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn read_only(&self) -> bool;
    /// JSON Schema (draft 2020-12) for the view's input type.
    fn input_schema(&self) -> Value;
    /// Deserialize raw JSON into the view's typed Params, run it,
    /// serialize the typed Output back to JSON.
    fn execute(&self, ctx: &Universe, raw: Value) -> Result<Value, ViewError>;
}

impl<V: View> DynView for V {
    fn name(&self) -> &'static str {
        V::NAME
    }

    fn description(&self) -> &'static str {
        V::DESCRIPTION
    }

    fn read_only(&self) -> bool {
        V::READ_ONLY
    }

    fn input_schema(&self) -> Value {
        let schema = schemars::schema_for!(V::Params);
        serde_json::to_value(&schema).expect("schema serializes")
    }

    fn execute(&self, ctx: &Universe, raw: Value) -> Result<Value, ViewError> {
        let params: V::Params = serde_json::from_value(raw)
            .map_err(|e| ViewError::InvalidParams(e.to_string()))?;
        let out = View::execute(self, ctx, params)?;
        serde_json::to_value(&out).map_err(|e| ViewError::Internal(e.to_string()))
    }
}

/// Flat list of every view in the running process. Built once at
/// startup by concatenating each service's `views()` output.
pub struct ViewRegistry {
    views: Vec<Arc<dyn DynView>>,
}

impl ViewRegistry {
    pub fn new() -> Self {
        ViewRegistry { views: Vec::new() }
    }

    /// Append the views from one service. Panics on duplicate names.
    pub fn extend(&mut self, more: Vec<Arc<dyn DynView>>) {
        for v in more {
            let name = v.name();
            if self.views.iter().any(|existing| existing.name() == name) {
                panic!("duplicate view name in registry: {}", name);
            }
            self.views.push(v);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn DynView>> {
        self.views.iter()
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }

    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// Look up a view by name. Returns `None` if absent.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn DynView>> {
        self.views.iter().find(|v| v.name() == name)
    }

    /// Names of every view, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.views.iter().map(|v| v.name()).collect()
    }
}

impl Default for ViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(JsonSchema, Deserialize, Serialize)]
    struct EchoParams {
        msg: String,
    }

    #[derive(Serialize)]
    struct EchoOut {
        echoed: String,
    }

    struct Echo;
    impl View for Echo {
        const NAME: &'static str = "echo";
        const DESCRIPTION: &'static str = "Echo the input message.";
        type Params = EchoParams;
        type Output = EchoOut;
        fn execute(&self, _ctx: &Universe, p: EchoParams) -> Result<EchoOut, ViewError> {
            Ok(EchoOut { echoed: p.msg })
        }
    }

    #[test]
    fn typed_view_round_trips_via_dynview() {
        let v: Arc<dyn DynView> = Arc::new(Echo);
        assert_eq!(v.name(), "echo");
        assert!(v.read_only());

        let universe = Universe::new();
        let result = v
            .execute(&universe, serde_json::json!({"msg": "hi"}))
            .expect("execute ok");
        assert_eq!(result["echoed"], "hi");
    }

    #[test]
    fn registry_rejects_duplicate_names() {
        let mut reg = ViewRegistry::new();
        reg.extend(vec![Arc::new(Echo)]);
        let result = std::panic::catch_unwind(|| {
            let mut r = ViewRegistry::new();
            r.extend(vec![Arc::new(Echo)]);
            r.extend(vec![Arc::new(Echo)]);
        });
        assert!(result.is_err());
    }

    #[test]
    fn registry_lookup_by_name() {
        let mut reg = ViewRegistry::new();
        reg.extend(vec![Arc::new(Echo)]);
        assert!(reg.get("echo").is_some());
        assert!(reg.get("nope").is_none());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn input_schema_is_well_formed() {
        let v: Arc<dyn DynView> = Arc::new(Echo);
        let schema = v.input_schema();
        // The blanket impl produces a JSON Schema object with at least
        // a "properties" field.
        assert!(schema.is_object());
        assert!(schema.get("properties").is_some());
    }
}
