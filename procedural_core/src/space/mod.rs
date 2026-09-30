//! Layer 1: Space — registration layer for attributes, relations, and contexts.
//!
//! A `Space<W: BitWord>` owns a `BitLayout<W>` plus registries of pure functions
//! that define the attributes, relations, and similarity contexts of a domain.

use crate::bits::BitLayout;
use crate::word::BitWord;
use chrono::{DateTime, Utc};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

pub mod arity;
pub(crate) mod attribute;
pub mod context;
pub mod entity;
pub mod error;
pub mod relation;

pub use arity::Arity;
pub use entity::EntitySnapshot;
pub use error::SpaceError;
pub use relation::RelatedIter;

use attribute::{AttributeSlot, EvalFn};
use context::{ContextDim, ContextSlot, Metric};
use error::type_label;
use relation::RelationSlot;

pub struct Space<W: BitWord> {
    name: String,
    layout: BitLayout<W>,
    attributes: HashMap<String, AttributeSlot<W>>,
    relations: HashMap<String, RelationSlot<W>>,
    contexts: HashMap<String, ContextSlot>,
}

impl<W: BitWord> Space<W> {
    pub fn new(name: &str, layout: BitLayout<W>) -> Self {
        Space {
            name: name.to_string(),
            layout,
            attributes: HashMap::new(),
            relations: HashMap::new(),
            contexts: HashMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn layout(&self) -> &BitLayout<W> {
        &self.layout
    }

    /// Register a static (non-temporal) attribute.
    pub fn attribute<V, F>(&mut self, name: &str, f: F) -> Result<(), SpaceError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(W) -> V + Send + Sync + 'static,
    {
        if self.attributes.contains_key(name) {
            return Err(SpaceError::DuplicateAttributeName(name.to_string()));
        }
        let boxed: Arc<dyn Fn(W) -> Box<dyn Any + Send + Sync> + Send + Sync> =
            Arc::new(move |id| Box::new(f(id)));
        self.attributes.insert(
            name.to_string(),
            AttributeSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: EvalFn::Static(boxed),
                indexable_field: None,
                composite_dep_fields: Vec::new(),
            },
        );
        Ok(())
    }

    /// Register a temporal attribute (depends on both id and time).
    pub fn temporal_attribute<V, F>(&mut self, name: &str, f: F) -> Result<(), SpaceError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(W, DateTime<Utc>) -> V + Send + Sync + 'static,
    {
        if self.attributes.contains_key(name) {
            return Err(SpaceError::DuplicateAttributeName(name.to_string()));
        }
        let boxed: Arc<dyn Fn(W, DateTime<Utc>) -> Box<dyn Any + Send + Sync> + Send + Sync> =
            Arc::new(move |id, t| Box::new(f(id, t)));
        self.attributes.insert(
            name.to_string(),
            AttributeSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: EvalFn::Temporal(boxed),
                indexable_field: None,
                composite_dep_fields: Vec::new(),
            },
        );
        Ok(())
    }

    /// Register an indexable attribute backed by a named bit-field.
    ///
    /// The bit-field must already exist in the space's `BitLayout`. `decode`
    /// maps the raw bit-pattern (u64) to the visible value type V. Indexable
    /// attributes are always static — the bits don't change with time.
    pub fn indexable_attribute<V, F>(
        &mut self,
        name: &str,
        field_name: &str,
        decode: F,
    ) -> Result<(), SpaceError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(u64) -> V + Send + Sync + 'static,
    {
        if self.attributes.contains_key(name) {
            return Err(SpaceError::DuplicateAttributeName(name.to_string()));
        }
        if !self.layout.has_field(field_name) {
            return Err(SpaceError::UnknownBitField(field_name.to_string()));
        }
        // Resolve offset/width at registration time so the closure doesn't
        // need to capture `BitLayout<W>` (which would require W: Send + Sync).
        let (offset, width) = self
            .layout
            .field_offset_width(field_name)
            .expect("field presence already validated above");
        let boxed: Arc<dyn Fn(W) -> Box<dyn Any + Send + Sync> + Send + Sync> =
            Arc::new(move |id: W| {
                let bits = id.extract_bits(offset, width);
                Box::new(decode(bits))
            });
        self.attributes.insert(
            name.to_string(),
            AttributeSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: EvalFn::Static(boxed),
                indexable_field: Some(field_name.to_string()),
                composite_dep_fields: Vec::new(),
            },
        );
        Ok(())
    }

    /// Register an indexable **composite** attribute: the decoder reads bits
    /// from `own_field` *and* from each field in `shared_fields` (in order),
    /// letting one attribute's value depend on multiple bit fields while
    /// every involved field remains independently queryable.
    ///
    /// `decode(own_bits, shared_bits: &[u64]) -> V` — `shared_bits[i]` is the
    /// current bit value of `shared_fields[i]`.
    ///
    /// A WHERE query on this attribute will (in the executor / reverse-lookup
    /// path) pin every involved field simultaneously: pushing down N separate
    /// `where_eq`s, one per field. All involved fields must already exist in
    /// the space's `BitLayout`.
    pub fn indexable_composite_attribute<V, F>(
        &mut self,
        name: &str,
        own_field: &str,
        shared_fields: &[&str],
        decode: F,
    ) -> Result<(), SpaceError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(u64, &[u64]) -> V + Send + Sync + 'static,
    {
        if self.attributes.contains_key(name) {
            return Err(SpaceError::DuplicateAttributeName(name.to_string()));
        }
        if !self.layout.has_field(own_field) {
            return Err(SpaceError::UnknownBitField(own_field.to_string()));
        }
        for f in shared_fields {
            if !self.layout.has_field(f) {
                return Err(SpaceError::UnknownBitField((*f).to_string()));
            }
        }
        #[allow(clippy::type_complexity)]
        let boxed: Arc<dyn Fn(u64, &[u64]) -> Box<dyn Any + Send + Sync> + Send + Sync> =
            Arc::new(move |own, shared| Box::new(decode(own, shared)));
        self.attributes.insert(
            name.to_string(),
            AttributeSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: EvalFn::Composite(boxed),
                indexable_field: Some(own_field.to_string()),
                composite_dep_fields: shared_fields.iter().map(|s| s.to_string()).collect(),
            },
        );
        Ok(())
    }

    /// Register a cross-space attribute — a pure function of (id, &World<W>) -> V.
    /// Requires the caller to route queries through World::attribute_value for dispatch.
    pub fn cross_space_attribute<V, F>(&mut self, name: &str, f: F) -> Result<(), SpaceError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(W, &crate::world::World<W>) -> V + Send + Sync + 'static,
    {
        if self.attributes.contains_key(name) {
            return Err(SpaceError::DuplicateAttributeName(name.to_string()));
        }
        #[allow(clippy::type_complexity)]
        let boxed: Arc<
            dyn Fn(W, &crate::world::World<W>) -> Box<dyn Any + Send + Sync> + Send + Sync,
        > = Arc::new(move |id, world| Box::new(f(id, world)));
        self.attributes.insert(
            name.to_string(),
            AttributeSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: EvalFn::CrossSpaceStatic(boxed),
                indexable_field: None,
                composite_dep_fields: Vec::new(),
            },
        );
        Ok(())
    }

    /// Register a cross-space temporal attribute — Fn(W, DateTime<Utc>, &World<W>) -> V.
    pub fn cross_space_temporal_attribute<V, F>(
        &mut self,
        name: &str,
        f: F,
    ) -> Result<(), SpaceError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(W, DateTime<Utc>, &crate::world::World<W>) -> V + Send + Sync + 'static,
    {
        if self.attributes.contains_key(name) {
            return Err(SpaceError::DuplicateAttributeName(name.to_string()));
        }
        #[allow(clippy::type_complexity)]
        let boxed: Arc<
            dyn Fn(W, DateTime<Utc>, &crate::world::World<W>) -> Box<dyn Any + Send + Sync>
                + Send
                + Sync,
        > = Arc::new(move |id, t, world| Box::new(f(id, t, world)));
        self.attributes.insert(
            name.to_string(),
            AttributeSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: EvalFn::CrossSpaceTemporal(boxed),
                indexable_field: None,
                composite_dep_fields: Vec::new(),
            },
        );
        Ok(())
    }

    /// Declare a relation: given an owner ID and time, enumerate `N` member IDs.
    /// Arity may be `Fixed(n)` (time-invariant shape) or `Dynamic(fn)` (owner+time
    /// dependent). `member_derive(owner, t, index)` returns the i-th member ID
    /// for `index in 0..N`.
    pub fn relation<D>(
        &mut self,
        name: &str,
        arity: Arity<W>,
        member_derive: D,
    ) -> Result<(), SpaceError>
    where
        D: Fn(W, DateTime<Utc>, usize) -> W + Send + Sync + 'static,
    {
        if self.relations.contains_key(name) {
            return Err(SpaceError::DuplicateRelationName(name.to_string()));
        }
        self.relations.insert(
            name.to_string(),
            RelationSlot {
                arity,
                member_derive: Arc::new(member_derive),
            },
        );
        Ok(())
    }

    pub fn has_relation(&self, name: &str) -> bool {
        self.relations.contains_key(name)
    }

    /// Declare a named similarity context.
    ///
    /// Each dim must reference an already-registered attribute whose value
    /// type is coercable to `f64` (f64/f32, u8..u64, i8..i64, bool) and
    /// whose `normalize` is `Normalization::None` (the only variant
    /// implemented in v0.1). Validation runs at registration so a
    /// later `candidates(...).take(...)` can't surface a panic on a
    /// type/normalization mismatch.
    pub fn context(
        &mut self,
        name: &str,
        dimensions: Vec<ContextDim>,
        metric: Metric,
    ) -> Result<(), SpaceError> {
        if self.contexts.contains_key(name) {
            return Err(SpaceError::DuplicateContextName(name.to_string()));
        }
        for dim in &dimensions {
            let slot = self.attributes.get(&dim.attribute).ok_or_else(|| {
                SpaceError::UnknownContextDimension {
                    context: name.to_string(),
                    attribute: dim.attribute.clone(),
                }
            })?;
            if !crate::coerce::is_coercable_numeric(slot.value_type_id) {
                return Err(SpaceError::UnsupportedDimType {
                    context: name.to_string(),
                    attribute: dim.attribute.clone(),
                    type_name: slot.value_type_name,
                });
            }
            if !matches!(dim.normalize, context::Normalization::None) {
                return Err(SpaceError::UnsupportedNormalization {
                    context: name.to_string(),
                    attribute: dim.attribute.clone(),
                    normalization: dim.normalize,
                });
            }
        }
        self.contexts
            .insert(name.to_string(), ContextSlot { dimensions, metric });
        Ok(())
    }

    pub fn has_context(&self, name: &str) -> bool {
        self.contexts.contains_key(name)
    }

    /// Internal accessor for a registered ContextSlot. `pub(crate)` —
    /// used by `search::candidates` to build query vectors.
    pub(crate) fn context_slot(&self, name: &str) -> Option<&crate::space::context::ContextSlot> {
        self.contexts.get(name)
    }

    /// Internal accessor for a registered AttributeSlot. `pub(crate)` —
    /// used by `search::candidates` to evaluate attributes.
    pub(crate) fn attribute_slot(
        &self,
        name: &str,
    ) -> Option<&crate::space::attribute::AttributeSlot<W>> {
        self.attributes.get(name)
    }

    /// Test-only: insert a `ContextSlot` without attribute-existence validation.
    /// Used to construct adversarial scenarios (e.g., dangling attribute references)
    /// that the normal `context()` API prevents.
    #[cfg(test)]
    pub(crate) fn insert_context_unchecked(&mut self, name: &str, slot: context::ContextSlot) {
        self.contexts.insert(name.to_string(), slot);
    }

    /// Evaluate all registered attributes for `id` at optional time `t`.
    /// Per-attribute failures (e.g., temporal attribute queried without t) are
    /// recorded in the snapshot's `errors` but do not short-circuit.
    pub fn entity(&self, id: W, t: Option<DateTime<Utc>>) -> EntitySnapshot<W> {
        let mut values: HashMap<String, Box<dyn Any + Send + Sync>> = HashMap::new();
        let mut errors = Vec::new();

        for (name, slot) in &self.attributes {
            match &slot.eval {
                EvalFn::Static(f) => {
                    values.insert(name.clone(), f(id));
                }
                EvalFn::Temporal(f) => match t {
                    Some(t) => {
                        values.insert(name.clone(), f(id, t));
                    }
                    None => {
                        errors.push((name.clone(), SpaceError::MissingTime { name: name.clone() }));
                    }
                },
                EvalFn::Composite(f) => {
                    let own_bits = match slot.indexable_field.as_ref() {
                        Some(field) => {
                            let (o, w) = self
                                .layout
                                .field_offset_width(field)
                                .expect("composite attribute's own_field must exist");
                            id.extract_bits(o, w)
                        }
                        None => 0,
                    };
                    let shared: Vec<u64> = slot
                        .composite_dep_fields
                        .iter()
                        .map(|f| {
                            let (o, w) = self
                                .layout
                                .field_offset_width(f)
                                .expect("composite attribute's shared field must exist");
                            id.extract_bits(o, w)
                        })
                        .collect();
                    values.insert(name.clone(), f(own_bits, &shared));
                }
                EvalFn::CrossSpaceStatic(_) | EvalFn::CrossSpaceTemporal(_) => {
                    errors.push((
                        name.clone(),
                        SpaceError::CrossSpaceRequiresWorld(name.clone()),
                    ));
                }
            }
        }

        EntitySnapshot { id, values, errors }
    }

    pub(crate) fn attributes_internal(&self) -> &HashMap<String, attribute::AttributeSlot<W>> {
        &self.attributes
    }

    pub fn attribute_names(&self) -> Vec<String> {
        self.attributes.keys().cloned().collect()
    }

    /// `TypeId` of an attribute's registered value type, or `None` if unknown.
    ///
    /// Lets external crates (notably `procedural_overlay`) validate write
    /// types against the declared schema before storing them. Works for
    /// every attribute kind — static, temporal, indexable, composite, and
    /// cross-space — since all variants share the same `value_type_id`
    /// field on their slot.
    pub fn attribute_type_id(&self, name: &str) -> Option<std::any::TypeId> {
        self.attributes.get(name).map(|s| s.value_type_id)
    }

    /// Static-display name of an attribute's value type, or `None` if unknown.
    /// Companion to [`Self::attribute_type_id`] for error messages.
    pub fn attribute_type_name(&self, name: &str) -> Option<&'static str> {
        self.attributes.get(name).map(|s| s.value_type_name)
    }

    /// All bit fields involved in an attribute's indexability: the `own`
    /// field first, then each `shared` field in declaration order. Returns
    /// `None` if the attribute is not registered, `Some(vec![])` if it exists
    /// but is not bit-indexable at all.
    pub fn attribute_indexable_fields(&self, attr_name: &str) -> Option<Vec<String>> {
        let slot = self.attributes.get(attr_name)?;
        let mut out = Vec::new();
        if let Some(own) = &slot.indexable_field {
            out.push(own.clone());
        }
        for f in &slot.composite_dep_fields {
            out.push(f.clone());
        }
        Some(out)
    }

    pub fn relation_names(&self) -> impl Iterator<Item = String> + '_ {
        self.relations.keys().cloned()
    }

    /// Returns `"fixed"` or `"dynamic"` for a registered relation, or `None`
    /// if the relation is not registered. Introspection helper.
    pub fn relation_arity_kind(&self, name: &str) -> Option<&'static str> {
        self.relations.get(name).map(|slot| slot.arity.kind())
    }

    pub fn context_names(&self) -> impl Iterator<Item = String> + '_ {
        self.contexts.keys().cloned()
    }

    /// Returns `Some(true)` if the attribute is temporal, `Some(false)` if static,
    /// or `None` if the attribute is not registered.
    pub fn is_temporal(&self, name: &str) -> Option<bool> {
        self.attributes.get(name).map(|slot| {
            matches!(
                slot.eval,
                EvalFn::Temporal(_) | EvalFn::CrossSpaceTemporal(_)
            )
        })
    }

    /// Query a single attribute's value. Type-checked against registration.
    pub fn attribute_value<V: Any + 'static>(
        &self,
        id: W,
        name: &str,
        t: Option<DateTime<Utc>>,
    ) -> Result<V, SpaceError> {
        let slot = self
            .attributes
            .get(name)
            .ok_or_else(|| SpaceError::UnknownAttribute(name.to_string()))?;

        if slot.value_type_id != TypeId::of::<V>() {
            return Err(SpaceError::AttributeTypeMismatch {
                name: name.to_string(),
                expected: type_label::<V>(),
                actual: slot.value_type_name,
            });
        }

        let boxed: Box<dyn Any + Send + Sync> = match &slot.eval {
            EvalFn::Static(f) => f(id),
            EvalFn::Temporal(f) => {
                let t = t.ok_or_else(|| SpaceError::MissingTime {
                    name: name.to_string(),
                })?;
                f(id, t)
            }
            EvalFn::Composite(f) => {
                let own_bits = match slot.indexable_field.as_ref() {
                    Some(field) => {
                        let (o, w) = self
                            .layout
                            .field_offset_width(field)
                            .expect("composite attribute's own_field must exist");
                        id.extract_bits(o, w)
                    }
                    None => 0,
                };
                let shared: Vec<u64> = slot
                    .composite_dep_fields
                    .iter()
                    .map(|fname| {
                        let (o, w) = self
                            .layout
                            .field_offset_width(fname)
                            .expect("composite attribute's shared field must exist");
                        id.extract_bits(o, w)
                    })
                    .collect();
                f(own_bits, &shared)
            }
            EvalFn::CrossSpaceStatic(_) | EvalFn::CrossSpaceTemporal(_) => {
                return Err(SpaceError::CrossSpaceRequiresWorld(name.to_string()));
            }
        };

        boxed
            .downcast::<V>()
            .map(|b| *b)
            .map_err(|_| SpaceError::AttributeTypeMismatch {
                name: name.to_string(),
                expected: type_label::<V>(),
                actual: slot.value_type_name,
            })
    }

    /// Enumerate the members of a registered relation for a given owner and time.
    /// The returned iterator yields exactly `N` items, where `N` comes from the
    /// relation's arity (fixed or dynamic).
    pub fn related(
        &self,
        owner_id: W,
        name: &str,
        t: DateTime<Utc>,
    ) -> Result<RelatedIter<'_, W>, SpaceError> {
        let slot = self
            .relations
            .get(name)
            .ok_or_else(|| SpaceError::UnknownRelation(name.to_string()))?;
        Ok(RelatedIter::new(slot, owner_id, t))
    }

    /// Start a `find()` query. The returned builder requires at least one
    /// indexable predicate (via `where_eq`, `where_range`, or `where_in`)
    /// before `.execute()` becomes callable — enforced at compile time
    /// via typestate.
    ///
    /// # Iteration order
    ///
    /// Results are enumerated in odometer order over the layout's fields:
    /// the **last-declared** field varies fastest, the first-declared varies
    /// slowest. Predicate constraints narrow each field's cursor, but
    /// unconstrained low-order fields (typically `entropy`) still walk their
    /// full `2^width` cycle between advances of higher-order fields.
    ///
    /// In practice: declare wide filler fields (entropy) **first**, and
    /// narrow keys you'll query on (e.g. `family_id`, `name_idx`) **last**,
    /// so a budgeted query produces varied results instead of cycling one
    /// filler before advancing the interesting axis.
    pub fn find(&self) -> crate::search::Query<'_, W, crate::search::Unbounded> {
        crate::search::Query::<W, crate::search::Unbounded>::new(self)
    }

    /// Entry point for the similarity query API.
    ///
    /// Returns a `CandidateQuery` builder; configure with `.budget`,
    /// `.envelope`, `.at`, `.min_score`, then consume with `.take(k)`.
    pub fn candidates<'a>(
        &'a self,
        id: W,
        context: &'a str,
    ) -> crate::search::CandidateQuery<'a, W> {
        crate::search::CandidateQuery {
            space: self,
            id,
            context,
            budget: crate::search::candidates::DEFAULT_BUDGET,
            envelope_field: None,
            time_anchor: None,
            min_score: f64::NEG_INFINITY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_space_has_expected_name_and_layout() {
        let layout = BitLayout::<u64>::new(vec![("field", 8)]).unwrap();
        let space = Space::<u64>::new("people", layout);
        assert_eq!(space.name(), "people");
        assert_eq!(space.layout().total_width(), 8);
    }

    #[test]
    fn works_with_u128() {
        let layout = BitLayout::<u128>::new(vec![("field", 64)]).unwrap();
        let space = Space::<u128>::new("rich_entities", layout);
        assert_eq!(space.name(), "rich_entities");
        assert_eq!(space.layout().total_width(), 64);
    }

    #[test]
    fn register_and_query_static_attribute() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("people", layout);

        space
            .attribute::<f64, _>("openness", |id: u64| {
                crate::hash::hash_float(id, "openness")
            })
            .unwrap();

        let v: f64 = space.attribute_value(42u64, "openness", None).unwrap();
        assert!((0.0..1.0).contains(&v));
    }

    #[test]
    fn duplicate_attribute_name_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("x", |_| 0.5).unwrap();
        let err = space.attribute::<f64, _>("x", |_| 0.5).unwrap_err();
        assert!(matches!(err, SpaceError::DuplicateAttributeName(ref n) if n == "x"));
    }

    #[test]
    fn unknown_attribute_lookup_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let space = Space::<u64>::new("p", layout);
        let err = space
            .attribute_value::<f64>(0u64, "nope", None)
            .unwrap_err();
        assert!(matches!(err, SpaceError::UnknownAttribute(ref n) if n == "nope"));
    }

    #[test]
    fn attribute_type_mismatch_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("openness", |_| 0.5).unwrap();
        let err = space
            .attribute_value::<i64>(0u64, "openness", None)
            .unwrap_err();
        assert!(matches!(err, SpaceError::AttributeTypeMismatch { .. }));
    }

    #[test]
    fn register_and_query_temporal_attribute() {
        use chrono::{Datelike, TimeZone};
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .temporal_attribute::<f64, _>("age", |id: u64, t: DateTime<Utc>| {
                let birth_year = 1950 + crate::hash::hash_int(id, "byr", 80) as i32;
                (t.year() - birth_year) as f64
            })
            .unwrap();

        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let age: f64 = space.attribute_value(42u64, "age", Some(t)).unwrap();
        assert!((0.0..100.0).contains(&age));
    }

    #[test]
    fn temporal_attribute_without_time_errors() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .temporal_attribute::<f64, _>("age", |_, _| 30.0)
            .unwrap();
        let err = space
            .attribute_value::<f64>(42u64, "age", None)
            .unwrap_err();
        assert!(matches!(err, SpaceError::MissingTime { ref name } if name == "age"));
    }

    #[test]
    fn static_attribute_ignores_time() {
        use chrono::TimeZone;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("openness", |_| 0.5).unwrap();
        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let v: f64 = space.attribute_value(0u64, "openness", Some(t)).unwrap();
        assert_eq!(v, 0.5);
    }

    #[test]
    fn register_and_query_indexable_attribute() {
        let layout = BitLayout::<u64>::new(vec![("home_locale_bits", 8), ("entropy", 56)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());

        space
            .indexable_attribute::<String, _>("home_locale", "home_locale_bits", |bits: u64| {
                format!("locale_{}", bits)
            })
            .unwrap();

        // Compose an id with home_locale_bits = 0xAB
        let id = layout.compose(&[("home_locale_bits", 0xAB)]);
        let locale: String = space.attribute_value(id, "home_locale", None).unwrap();
        assert_eq!(locale, "locale_171");
    }

    #[test]
    fn indexable_unknown_bit_field_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        let err = space
            .indexable_attribute::<String, _>("bad", "nonexistent", |_| String::new())
            .unwrap_err();
        assert!(matches!(err, SpaceError::UnknownBitField(ref n) if n == "nonexistent"));
    }

    #[test]
    fn indexable_attribute_duplicate_name_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("x", |_| 0.5).unwrap();
        let err = space
            .indexable_attribute::<u64, _>("x", "x", |b| b)
            .unwrap_err();
        assert!(matches!(err, SpaceError::DuplicateAttributeName(_)));
    }

    #[test]
    fn register_relation() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .relation("parents", Arity::Fixed(2), |_owner, _t, _idx| 0u64)
            .unwrap();
        assert!(space.has_relation("parents"));
    }

    #[test]
    fn duplicate_relation_name_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .relation("parents", Arity::Fixed(2), |_, _, _| 0u64)
            .unwrap();
        let err = space
            .relation("parents", Arity::Fixed(1), |_, _, _| 0u64)
            .unwrap_err();
        assert!(matches!(err, SpaceError::DuplicateRelationName(ref n) if n == "parents"));
    }

    #[test]
    fn related_enumerates_fixed_arity() {
        use chrono::TimeZone;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut families = Space::<u64>::new("families", layout);
        families
            .relation("parents", Arity::Fixed(2), |owner, _t, idx| {
                owner ^ (idx as u64 + 1)
            })
            .unwrap();

        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        let ids: Vec<u64> = families.related(100u64, "parents", t).unwrap().collect();
        assert_eq!(ids, vec![100u64 ^ 1, 100u64 ^ 2]);
    }

    #[test]
    fn related_enumerates_dynamic_arity() {
        use chrono::TimeZone;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut families = Space::<u64>::new("families", layout);
        families
            .relation(
                "children",
                Arity::dynamic(|owner, _t| (owner % 4) as usize),
                |owner, _t, idx| owner + (idx as u64 + 1) * 10,
            )
            .unwrap();

        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        // owner=7 -> count=3
        let ids: Vec<u64> = families.related(7u64, "children", t).unwrap().collect();
        assert_eq!(ids, vec![17, 27, 37]);
        // owner=8 -> count=0
        let ids: Vec<u64> = families.related(8u64, "children", t).unwrap().collect();
        assert_eq!(ids, Vec::<u64>::new());
    }

    #[test]
    fn related_unknown_relation_rejected() {
        use chrono::TimeZone;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let space = Space::<u64>::new("p", layout);
        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        let err = space.related(0u64, "missing", t).unwrap_err();
        assert!(matches!(err, SpaceError::UnknownRelation(ref n) if n == "missing"));
    }

    #[test]
    fn related_iterator_reports_size_hint() {
        use chrono::TimeZone;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .relation("parents", Arity::Fixed(2), |_, _, _| 0u64)
            .unwrap();
        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        let iter = space.related(0u64, "parents", t).unwrap();
        assert_eq!(iter.size_hint(), (2, Some(2)));
        assert_eq!(iter.len(), 2);
    }

    #[test]
    fn register_context() {
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("openness", |_| 0.5).unwrap();
        space
            .context(
                "friendship",
                vec![ContextDim {
                    attribute: "openness".into(),
                    weight: 1.0,
                    normalize: Normalization::None,
                }],
                Metric::Cosine,
            )
            .unwrap();
        assert!(space.has_context("friendship"));
    }

    #[test]
    fn context_unknown_attribute_rejected() {
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        let err = space
            .context(
                "friendship",
                vec![ContextDim {
                    attribute: "nope".into(),
                    weight: 1.0,
                    normalize: Normalization::None,
                }],
                Metric::Cosine,
            )
            .unwrap_err();
        assert!(matches!(err, SpaceError::UnknownContextDimension { .. }));
    }

    #[test]
    fn context_with_non_numeric_dim_type_rejected() {
        // A String-typed attribute can't be coerced to f64 by the
        // similarity scorer. Registration must reject — the panic that
        // used to surface deep inside `candidates().take()` is not a
        // good UX.
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("locale_bits", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .indexable_attribute::<String, _>("locale", "locale_bits", |b| format!("L{}", b))
            .unwrap();
        let err = space
            .context(
                "ctx",
                vec![ContextDim {
                    attribute: "locale".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                }],
                Metric::Cosine,
            )
            .unwrap_err();
        assert!(
            matches!(
                &err,
                SpaceError::UnsupportedDimType { context, attribute, .. }
                    if context == "ctx" && attribute == "locale"
            ),
            "wrong error: {:?}",
            err
        );
    }

    #[test]
    fn context_with_unsupported_normalization_rejected() {
        // Only Normalization::None is implemented in v0.1. Registering
        // a context with ZScore must error rather than registering and
        // panicking later during `take()`.
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("a", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("score", |_| 0.5).unwrap();
        let err = space
            .context(
                "ctx",
                vec![ContextDim {
                    attribute: "score".to_string(),
                    weight: 1.0,
                    normalize: Normalization::ZScore,
                }],
                Metric::Cosine,
            )
            .unwrap_err();
        assert!(
            matches!(
                &err,
                SpaceError::UnsupportedNormalization {
                    context, attribute, normalization: Normalization::ZScore
                } if context == "ctx" && attribute == "score"
            ),
            "wrong error: {:?}",
            err
        );
    }

    #[test]
    fn context_dim_validation_runs_per_dim() {
        // Dim ordering: first dim ok, second dim has unsupported type → error
        // pinpoints the offending attribute, not the first one.
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("a", 8), ("name_bits", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("score", |_| 0.5).unwrap();
        space
            .indexable_attribute::<String, _>("name", "name_bits", |b| format!("n{}", b))
            .unwrap();
        let err = space
            .context(
                "ctx",
                vec![
                    ContextDim {
                        attribute: "score".to_string(),
                        weight: 1.0,
                        normalize: Normalization::None,
                    },
                    ContextDim {
                        attribute: "name".to_string(),
                        weight: 1.0,
                        normalize: Normalization::None,
                    },
                ],
                Metric::Cosine,
            )
            .unwrap_err();
        assert!(
            matches!(
                &err,
                SpaceError::UnsupportedDimType { attribute, .. } if attribute == "name"
            ),
            "wrong error: {:?}",
            err
        );
    }

    #[test]
    fn context_with_all_numeric_types_accepted() {
        // Every type the scorer can coerce must register cleanly.
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("a_f64", |_| 0.0).unwrap();
        space.attribute::<f32, _>("a_f32", |_| 0.0).unwrap();
        space.attribute::<u64, _>("a_u64", |_| 0).unwrap();
        space.attribute::<u32, _>("a_u32", |_| 0).unwrap();
        space.attribute::<u16, _>("a_u16", |_| 0).unwrap();
        space.attribute::<u8, _>("a_u8", |_| 0).unwrap();
        space.attribute::<i64, _>("a_i64", |_| 0).unwrap();
        space.attribute::<i32, _>("a_i32", |_| 0).unwrap();
        space.attribute::<i16, _>("a_i16", |_| 0).unwrap();
        space.attribute::<i8, _>("a_i8", |_| 0).unwrap();
        space.attribute::<bool, _>("a_bool", |_| false).unwrap();
        let dims = [
            "a_f64", "a_f32", "a_u64", "a_u32", "a_u16", "a_u8", "a_i64", "a_i32", "a_i16", "a_i8",
            "a_bool",
        ]
        .iter()
        .map(|n| ContextDim {
            attribute: (*n).to_string(),
            weight: 1.0,
            normalize: Normalization::None,
        })
        .collect();
        space
            .context("everything", dims, Metric::Euclidean)
            .unwrap();
        assert!(space.has_context("everything"));
    }

    #[test]
    fn duplicate_context_name_rejected() {
        use crate::space::context::{ContextDim, Metric, Normalization};
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("a", |_| 0.5).unwrap();
        let dims = vec![ContextDim {
            attribute: "a".into(),
            weight: 1.0,
            normalize: Normalization::None,
        }];
        space.context("c", dims.clone(), Metric::Cosine).unwrap();
        let err = space.context("c", dims, Metric::Cosine).unwrap_err();
        assert!(matches!(err, SpaceError::DuplicateContextName(ref n) if n == "c"));
    }

    #[test]
    fn entity_snapshot_collects_all_attributes() {
        let layout = BitLayout::<u64>::new(vec![("locale_bits", 8), ("x", 56)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());
        space.attribute::<f64, _>("openness", |_| 0.7).unwrap();
        space
            .indexable_attribute::<String, _>("locale", "locale_bits", |b| format!("l{}", b))
            .unwrap();

        let id = layout.compose(&[("locale_bits", 5)]);
        let snap = space.entity(id, None);
        assert_eq!(snap.id(), id);

        let o: &f64 = snap.get("openness").unwrap();
        assert_eq!(*o, 0.7);
        let l: &String = snap.get("locale").unwrap();
        assert_eq!(l, "l5");
    }

    #[test]
    fn entity_snapshot_records_temporal_errors_when_t_is_missing() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .temporal_attribute::<f64, _>("age", |_, _| 30.0)
            .unwrap();
        let snap = space.entity(0u64, None);
        assert!(snap.errors().iter().any(|(n, _)| n == "age"));
        assert!(snap.get::<f64>("age").is_err());
    }

    #[test]
    fn entity_snapshot_temporal_with_time_succeeds() {
        use chrono::TimeZone;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .temporal_attribute::<f64, _>("age", |_, _| 30.0)
            .unwrap();
        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let snap = space.entity(0u64, Some(t));
        assert_eq!(*snap.get::<f64>("age").unwrap(), 30.0);
        assert!(snap.errors().is_empty());
    }

    #[test]
    fn introspection_enumerates_registered_items() {
        use crate::space::context::{ContextDim, Metric, Normalization};

        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("openness", |_| 0.5).unwrap();
        space.attribute::<f64, _>("extraversion", |_| 0.5).unwrap();
        space
            .temporal_attribute::<f64, _>("age", |_, _| 30.0)
            .unwrap();
        space
            .relation("parents", Arity::Fixed(2), |_, _, _| 0u64)
            .unwrap();
        space
            .context(
                "friendship",
                vec![
                    ContextDim {
                        attribute: "openness".into(),
                        weight: 1.0,
                        normalize: Normalization::None,
                    },
                    ContextDim {
                        attribute: "extraversion".into(),
                        weight: 1.0,
                        normalize: Normalization::None,
                    },
                ],
                Metric::Cosine,
            )
            .unwrap();

        let mut attr_names = space.attribute_names();
        attr_names.sort();
        assert_eq!(
            attr_names,
            vec![
                "age".to_string(),
                "extraversion".to_string(),
                "openness".to_string()
            ]
        );

        let rel_names: Vec<_> = space.relation_names().collect();
        assert_eq!(rel_names, vec!["parents".to_string()]);

        let ctx_names: Vec<_> = space.context_names().collect();
        assert_eq!(ctx_names, vec!["friendship".to_string()]);
    }

    #[test]
    fn is_temporal_reports_correctly() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("s", |_| 0.5).unwrap();
        space
            .temporal_attribute::<f64, _>("tm", |_, _| 0.5)
            .unwrap();
        assert_eq!(space.is_temporal("s"), Some(false));
        assert_eq!(space.is_temporal("tm"), Some(true));
        assert_eq!(space.is_temporal("missing"), None);
    }

    #[test]
    fn attribute_type_id_reports_registered_type_for_every_kind() {
        // Cover all six EvalFn variants. A sister crate (procedural_overlay)
        // uses these accessors to validate write types against the declared
        // attribute schema, so every registration path must surface its type.
        use crate::world::World;
        use std::any::TypeId;
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4), ("rest", 56)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        // Static
        space.attribute::<f64, _>("static_f64", |_| 0.0).unwrap();
        // Temporal
        space
            .temporal_attribute::<i64, _>("temporal_i64", |_, _| 0)
            .unwrap();
        // Indexable
        space
            .indexable_attribute::<u64, _>("indexable_u64", "own", |b| b)
            .unwrap();
        // Composite
        space
            .indexable_composite_attribute::<String, _>(
                "composite_str",
                "own",
                &["shared"],
                |a, _| a.to_string(),
            )
            .unwrap();
        // CrossSpaceStatic
        space
            .cross_space_attribute::<bool, _>("cross_static_bool", |_, _: &World<u64>| true)
            .unwrap();
        // CrossSpaceTemporal
        space
            .cross_space_temporal_attribute::<f32, _>(
                "cross_temporal_f32",
                |_, _, _: &World<u64>| 0.0f32,
            )
            .unwrap();

        assert_eq!(
            space.attribute_type_id("static_f64"),
            Some(TypeId::of::<f64>())
        );
        assert_eq!(
            space.attribute_type_id("temporal_i64"),
            Some(TypeId::of::<i64>())
        );
        assert_eq!(
            space.attribute_type_id("indexable_u64"),
            Some(TypeId::of::<u64>())
        );
        assert_eq!(
            space.attribute_type_id("composite_str"),
            Some(TypeId::of::<String>())
        );
        assert_eq!(
            space.attribute_type_id("cross_static_bool"),
            Some(TypeId::of::<bool>())
        );
        assert_eq!(
            space.attribute_type_id("cross_temporal_f32"),
            Some(TypeId::of::<f32>())
        );
        assert_eq!(space.attribute_type_id("missing"), None);
    }

    #[test]
    fn attribute_type_name_reports_registered_type_name_for_every_kind() {
        // Companion to attribute_type_id; gives a static-display name
        // suitable for inclusion in error messages.
        use crate::world::World;
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("a", |_| 0.0).unwrap();
        space.temporal_attribute::<i64, _>("b", |_, _| 0).unwrap();
        space
            .indexable_attribute::<u64, _>("c", "own", |b| b)
            .unwrap();
        space
            .indexable_composite_attribute::<String, _>("d", "own", &["shared"], |_, _| {
                String::new()
            })
            .unwrap();
        space
            .cross_space_attribute::<bool, _>("e", |_, _: &World<u64>| true)
            .unwrap();
        space
            .cross_space_temporal_attribute::<f32, _>("f", |_, _, _: &World<u64>| 0.0f32)
            .unwrap();

        assert_eq!(
            space.attribute_type_name("a"),
            Some(std::any::type_name::<f64>())
        );
        assert_eq!(
            space.attribute_type_name("b"),
            Some(std::any::type_name::<i64>())
        );
        assert_eq!(
            space.attribute_type_name("c"),
            Some(std::any::type_name::<u64>())
        );
        assert_eq!(
            space.attribute_type_name("d"),
            Some(std::any::type_name::<String>())
        );
        assert_eq!(
            space.attribute_type_name("e"),
            Some(std::any::type_name::<bool>())
        );
        assert_eq!(
            space.attribute_type_name("f"),
            Some(std::any::type_name::<f32>())
        );
        assert_eq!(space.attribute_type_name("missing"), None);
    }

    #[test]
    fn register_cross_space_attribute() {
        use crate::world::World;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("people", layout);
        // Cross-space functions take &World<W>; we just need the signature to compile.
        space
            .cross_space_attribute::<f64, _>("derived", |id: u64, _world: &World<u64>| {
                id as f64 * 0.5
            })
            .unwrap();
        assert!(space.is_temporal("derived") == Some(false));
    }

    #[test]
    fn register_cross_space_temporal_attribute() {
        use crate::world::World;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("people", layout);
        space
            .cross_space_temporal_attribute::<f64, _>(
                "derived_t",
                |id: u64, t: DateTime<Utc>, _world: &World<u64>| id as f64 + t.timestamp() as f64,
            )
            .unwrap();
        assert!(space.is_temporal("derived_t") == Some(true));
    }

    #[test]
    fn duplicate_cross_space_name_rejected() {
        use crate::world::World;
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("x", |_| 0.5).unwrap();
        let err = space
            .cross_space_attribute::<f64, _>("x", |_, _: &World<u64>| 0.5)
            .unwrap_err();
        assert!(matches!(err, SpaceError::DuplicateAttributeName(_)));
    }

    // ---------------------------- composite attributes ------------

    #[test]
    fn register_and_query_composite_attribute() {
        let layout =
            BitLayout::<u64>::new(vec![("own", 4), ("shared", 4), ("entropy", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());

        // Decoder reads own + shared and returns a synthetic string.
        space
            .indexable_composite_attribute::<String, _>(
                "label",
                "own",
                &["shared"],
                |own, shared| format!("{}-{}", own, shared[0]),
            )
            .unwrap();

        // own=3, shared=7 → "3-7"
        let id = layout.compose(&[("own", 3), ("shared", 7)]);
        let v: String = space.attribute_value(id, "label", None).unwrap();
        assert_eq!(v, "3-7");

        // own=15, shared=0 → "15-0" — confirms both bits read independently.
        let id = layout.compose(&[("own", 15), ("shared", 0)]);
        let v: String = space.attribute_value(id, "label", None).unwrap();
        assert_eq!(v, "15-0");
    }

    #[test]
    fn composite_attribute_with_multiple_shared_fields() {
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("a", 4), ("b", 4), ("c", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());

        space
            .indexable_composite_attribute::<String, _>(
                "combo",
                "own",
                &["a", "b", "c"],
                |own, shared| {
                    assert_eq!(shared.len(), 3);
                    format!("{}|{},{},{}", own, shared[0], shared[1], shared[2])
                },
            )
            .unwrap();

        let id = layout.compose(&[("own", 1), ("a", 2), ("b", 3), ("c", 4)]);
        let v: String = space.attribute_value(id, "combo", None).unwrap();
        assert_eq!(v, "1|2,3,4");
    }

    #[test]
    fn composite_attribute_duplicate_name_rejected() {
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space.attribute::<f64, _>("dup", |_| 0.0).unwrap();
        let err = space
            .indexable_composite_attribute::<String, _>("dup", "own", &["shared"], |_, _| {
                String::new()
            })
            .unwrap_err();
        assert!(matches!(err, SpaceError::DuplicateAttributeName(ref n) if n == "dup"));
    }

    #[test]
    fn composite_attribute_unknown_own_field_rejected() {
        let layout = BitLayout::<u64>::new(vec![("x", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        let err = space
            .indexable_composite_attribute::<String, _>("bad", "nonexistent", &["x"], |_, _| {
                String::new()
            })
            .unwrap_err();
        assert!(matches!(err, SpaceError::UnknownBitField(ref n) if n == "nonexistent"));
    }

    #[test]
    fn composite_attribute_unknown_shared_field_rejected() {
        let layout = BitLayout::<u64>::new(vec![("own", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        let err = space
            .indexable_composite_attribute::<String, _>("bad", "own", &["missing_dep"], |_, _| {
                String::new()
            })
            .unwrap_err();
        assert!(matches!(err, SpaceError::UnknownBitField(ref n) if n == "missing_dep"));
    }

    #[test]
    fn composite_attribute_with_no_shared_fields_is_equivalent_to_indexable() {
        // Empty shared_fields list should work — the decoder just ignores
        // the shared slice and behaves like a regular indexable attribute.
        let layout = BitLayout::<u64>::new(vec![("own", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());
        space
            .indexable_composite_attribute::<String, _>("lonely", "own", &[], |own, shared| {
                assert!(shared.is_empty());
                format!("v{}", own)
            })
            .unwrap();
        let id = layout.compose(&[("own", 5)]);
        let v: String = space.attribute_value(id, "lonely", None).unwrap();
        assert_eq!(v, "v5");
    }

    #[test]
    fn composite_attribute_type_mismatch_rejected() {
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());
        space
            .indexable_composite_attribute::<String, _>("label", "own", &["shared"], |own, _| {
                format!("{}", own)
            })
            .unwrap();
        let id = layout.compose(&[("own", 1), ("shared", 0)]);
        let err = space.attribute_value::<u64>(id, "label", None).unwrap_err();
        assert!(matches!(err, SpaceError::AttributeTypeMismatch { .. }));
    }

    #[test]
    fn attribute_indexable_fields_reports_deps() {
        let layout =
            BitLayout::<u64>::new(vec![("own", 4), ("a", 4), ("b", 4), ("hash_attr", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);

        space
            .indexable_composite_attribute::<String, _>("composite", "own", &["a", "b"], |_, _| {
                String::new()
            })
            .unwrap();
        space
            .indexable_attribute::<String, _>("single", "a", |_| String::new())
            .unwrap();
        space.attribute::<f64, _>("not_indexable", |_| 0.0).unwrap();

        // Composite: own first, then deps in declaration order.
        assert_eq!(
            space.attribute_indexable_fields("composite"),
            Some(vec!["own".into(), "a".into(), "b".into()])
        );
        // Single: just the own field.
        assert_eq!(
            space.attribute_indexable_fields("single"),
            Some(vec!["a".into()])
        );
        // Non-indexable: Some(empty), not None — the attribute exists.
        assert_eq!(
            space.attribute_indexable_fields("not_indexable"),
            Some(Vec::new())
        );
        // Unknown: None.
        assert_eq!(space.attribute_indexable_fields("missing"), None);
    }

    #[test]
    fn composite_is_not_temporal() {
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .indexable_composite_attribute::<String, _>("label", "own", &["shared"], |_, _| {
                String::new()
            })
            .unwrap();
        assert_eq!(space.is_temporal("label"), Some(false));
    }

    #[test]
    fn entity_snapshot_includes_composite_value() {
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());
        space
            .indexable_composite_attribute::<String, _>(
                "label",
                "own",
                &["shared"],
                |own, shared| format!("{}-{}", own, shared[0]),
            )
            .unwrap();

        let id = layout.compose(&[("own", 9), ("shared", 2)]);
        let snap = space.entity(id, None);
        assert_eq!(snap.get::<String>("label").unwrap(), "9-2");
        assert!(snap.errors().is_empty());
    }

    #[test]
    fn composite_value_depends_on_shared_bits() {
        // Changing *only* the shared-field bits changes the decoded value —
        // confirms the decoder really reads them, not just own_field.
        let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());
        space
            .indexable_composite_attribute::<u64, _>("sum", "own", &["shared"], |own, shared| {
                own + shared[0]
            })
            .unwrap();

        let id_a = layout.compose(&[("own", 3), ("shared", 1)]);
        let id_b = layout.compose(&[("own", 3), ("shared", 5)]);
        let va: u64 = space.attribute_value(id_a, "sum", None).unwrap();
        let vb: u64 = space.attribute_value(id_b, "sum", None).unwrap();
        assert_eq!(va, 4);
        assert_eq!(vb, 8);
    }

    #[test]
    fn two_composites_can_share_the_same_dependency_field() {
        // `country_idx` is read by both `first_name` and `last_name` in
        // internot_sql — make sure that pattern works at the framework level.
        let layout =
            BitLayout::<u64>::new(vec![("a_own", 4), ("b_own", 4), ("shared", 4)]).unwrap();
        let mut space = Space::<u64>::new("p", layout.clone());
        space
            .indexable_composite_attribute::<String, _>("a", "a_own", &["shared"], |own, s| {
                format!("A:{}:{}", own, s[0])
            })
            .unwrap();
        space
            .indexable_composite_attribute::<String, _>("b", "b_own", &["shared"], |own, s| {
                format!("B:{}:{}", own, s[0])
            })
            .unwrap();

        let id = layout.compose(&[("a_own", 1), ("b_own", 2), ("shared", 3)]);
        assert_eq!(
            space.attribute_value::<String>(id, "a", None).unwrap(),
            "A:1:3"
        );
        assert_eq!(
            space.attribute_value::<String>(id, "b", None).unwrap(),
            "B:2:3"
        );
    }

    #[test]
    fn relation_arity_kind_reports_variant() {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut space = Space::<u64>::new("p", layout);
        space
            .relation("parents", Arity::Fixed(2), |_, _, _| 0u64)
            .unwrap();
        space
            .relation("children", Arity::dynamic(|_, _| 3), |_, _, _| 0u64)
            .unwrap();

        assert_eq!(space.relation_arity_kind("parents"), Some("fixed"));
        assert_eq!(space.relation_arity_kind("children"), Some("dynamic"));
        assert_eq!(space.relation_arity_kind("missing"), None);
    }
}
