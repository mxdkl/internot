//! Attribute registration and query internals for `Space`.

use crate::word::BitWord;
use crate::world::World;
use chrono::{DateTime, Utc};
use std::any::{Any, TypeId};
use std::sync::Arc;

pub(crate) enum EvalFn<W: BitWord> {
    Static(Arc<dyn Fn(W) -> Box<dyn Any + Send + Sync> + Send + Sync>),
    Temporal(Arc<dyn Fn(W, DateTime<Utc>) -> Box<dyn Any + Send + Sync> + Send + Sync>),
    #[allow(clippy::type_complexity)]
    CrossSpaceStatic(Arc<dyn Fn(W, &World<W>) -> Box<dyn Any + Send + Sync> + Send + Sync>),
    #[allow(clippy::type_complexity)]
    CrossSpaceTemporal(
        Arc<dyn Fn(W, DateTime<Utc>, &World<W>) -> Box<dyn Any + Send + Sync> + Send + Sync>,
    ),
    /// Composite indexable attribute: decoder reads its own field's bits plus
    /// the bits of one or more shared fields, in the order given in
    /// `AttributeSlot::composite_dep_fields`.
    #[allow(clippy::type_complexity)]
    Composite(Arc<dyn Fn(u64, &[u64]) -> Box<dyn Any + Send + Sync> + Send + Sync>),
}

pub(crate) struct AttributeSlot<W: BitWord> {
    pub(crate) value_type_id: TypeId,
    pub(crate) value_type_name: &'static str,
    pub(crate) eval: EvalFn<W>,
    #[allow(dead_code)]
    pub(crate) indexable_field: Option<String>,
    /// For `EvalFn::Composite`: the shared (dependency) fields the decoder
    /// reads *in addition* to `indexable_field`. Empty for non-composite.
    pub(crate) composite_dep_fields: Vec<String>,
}
