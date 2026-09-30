//! World — cross-space container owning multiple `Space<W>` instances and global time functions.

use crate::space::Space;
use crate::time::default_epoch;
use crate::word::BitWord;
use chrono::{DateTime, Utc};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::space::error::type_label;
use crate::space::RelatedIter;
use crate::space::SpaceError;

#[derive(Debug)]
pub enum WorldError {
    DuplicateSpaceName(String),
    UnknownSpace(String),
    DuplicateGlobalName(String),
    UnknownGlobal(String),
    GlobalTypeMismatch {
        name: String,
        expected: &'static str,
        actual: &'static str,
    },
    SpaceError(Box<SpaceError>),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::DuplicateSpaceName(n) => write!(f, "duplicate space name: {}", n),
            WorldError::UnknownSpace(n) => write!(f, "unknown space: {}", n),
            WorldError::DuplicateGlobalName(n) => write!(f, "duplicate global name: {}", n),
            WorldError::UnknownGlobal(n) => write!(f, "unknown global: {}", n),
            WorldError::GlobalTypeMismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "global {} type mismatch: expected {}, got {}",
                name, expected, actual
            ),
            WorldError::SpaceError(e) => write!(f, "space error: {}", e),
        }
    }
}

impl std::error::Error for WorldError {}

impl From<SpaceError> for WorldError {
    fn from(e: SpaceError) -> Self {
        WorldError::SpaceError(Box::new(e))
    }
}

pub(crate) struct GlobalSlot {
    pub(crate) value_type_id: TypeId,
    pub(crate) value_type_name: &'static str,
    pub(crate) eval: Arc<dyn Fn(DateTime<Utc>) -> Box<dyn Any + Send + Sync> + Send + Sync>,
}

pub struct World<W: BitWord> {
    spaces: HashMap<String, Space<W>>,
    globals: HashMap<String, GlobalSlot>,
    epoch: DateTime<Utc>,
}

impl<W: BitWord> Default for World<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: BitWord> World<W> {
    pub fn new() -> Self {
        World {
            spaces: HashMap::new(),
            globals: HashMap::new(),
            epoch: default_epoch(),
        }
    }

    /// Construct a World pinned to a specific "day 0" anchor. Use this
    /// to shift the simulation in time without changing any procedural
    /// outputs — `f(id, key)` is unchanged; only absolute timestamps
    /// derived from `day_offset` shift by the delta from `default_epoch`.
    pub fn with_epoch(epoch: DateTime<Utc>) -> Self {
        World {
            spaces: HashMap::new(),
            globals: HashMap::new(),
            epoch,
        }
    }

    /// The "day 0" anchor for this world. Callers that derive absolute
    /// timestamps from a stored `day_offset` should compose against
    /// this rather than a per-crate constant.
    #[inline]
    pub fn epoch(&self) -> DateTime<Utc> {
        self.epoch
    }

    /// Override the epoch on an existing World.
    pub fn set_epoch(&mut self, epoch: DateTime<Utc>) {
        self.epoch = epoch;
    }

    pub fn register(&mut self, space: Space<W>) -> Result<(), WorldError> {
        let name = space.name().to_string();
        if self.spaces.contains_key(&name) {
            return Err(WorldError::DuplicateSpaceName(name));
        }
        self.spaces.insert(name, space);
        Ok(())
    }

    pub fn space(&self, name: &str) -> Result<&Space<W>, WorldError> {
        self.spaces
            .get(name)
            .ok_or_else(|| WorldError::UnknownSpace(name.to_string()))
    }

    pub fn has_space(&self, name: &str) -> bool {
        self.spaces.contains_key(name)
    }

    pub fn space_names(&self) -> impl Iterator<Item = String> + '_ {
        self.spaces.keys().cloned()
    }

    /// Query an attribute's value on a named space. Routes cross-space attributes
    /// by passing `self` back into their eval function.
    pub fn attribute_value<V: Any + 'static>(
        &self,
        space_name: &str,
        id: W,
        attr_name: &str,
        t: Option<DateTime<Utc>>,
    ) -> Result<V, WorldError> {
        let space = self.space(space_name)?;
        let slot = space.attributes_internal().get(attr_name).ok_or_else(|| {
            WorldError::SpaceError(Box::new(SpaceError::UnknownAttribute(
                attr_name.to_string(),
            )))
        })?;

        if slot.value_type_id != TypeId::of::<V>() {
            return Err(WorldError::SpaceError(Box::new(
                SpaceError::AttributeTypeMismatch {
                    name: attr_name.to_string(),
                    expected: type_label::<V>(),
                    actual: slot.value_type_name,
                },
            )));
        }

        use crate::space::attribute::EvalFn;
        let boxed: Box<dyn Any + Send + Sync> = match &slot.eval {
            EvalFn::Static(f) => f(id),
            EvalFn::Temporal(f) => {
                let t = t.ok_or_else(|| {
                    WorldError::SpaceError(Box::new(SpaceError::MissingTime {
                        name: attr_name.to_string(),
                    }))
                })?;
                f(id, t)
            }
            EvalFn::Composite(f) => {
                let layout = space.layout();
                let own_bits = match slot.indexable_field.as_ref() {
                    Some(fname) => {
                        let (o, w) = layout
                            .field_offset_width(fname)
                            .expect("composite own_field must exist");
                        id.extract_bits(o, w)
                    }
                    None => 0,
                };
                let shared: Vec<u64> = slot
                    .composite_dep_fields
                    .iter()
                    .map(|fname| {
                        let (o, w) = layout
                            .field_offset_width(fname)
                            .expect("composite shared field must exist");
                        id.extract_bits(o, w)
                    })
                    .collect();
                f(own_bits, &shared)
            }
            EvalFn::CrossSpaceStatic(f) => f(id, self),
            EvalFn::CrossSpaceTemporal(f) => {
                let t = t.ok_or_else(|| {
                    WorldError::SpaceError(Box::new(SpaceError::MissingTime {
                        name: attr_name.to_string(),
                    }))
                })?;
                f(id, t, self)
            }
        };

        boxed.downcast::<V>().map(|b| *b).map_err(|_| {
            WorldError::SpaceError(Box::new(SpaceError::AttributeTypeMismatch {
                name: attr_name.to_string(),
                expected: type_label::<V>(),
                actual: slot.value_type_name,
            }))
        })
    }

    /// Evaluate all attributes (including cross-space) for an id on a named space.
    pub fn entity(
        &self,
        space_name: &str,
        id: W,
        t: Option<DateTime<Utc>>,
    ) -> Result<crate::space::EntitySnapshot<W>, WorldError> {
        use crate::space::attribute::EvalFn;
        let space = self.space(space_name)?;
        let mut values: HashMap<String, Box<dyn Any + Send + Sync>> = HashMap::new();
        let mut errors: Vec<(String, SpaceError)> = Vec::new();

        for (name, slot) in space.attributes_internal().iter() {
            match &slot.eval {
                EvalFn::Static(f) => {
                    values.insert(name.clone(), f(id));
                }
                EvalFn::Temporal(f) => match t {
                    Some(t) => {
                        values.insert(name.clone(), f(id, t));
                    }
                    None => {
                        errors.push((name.clone(), SpaceError::MissingTime { name: name.clone() }))
                    }
                },
                EvalFn::Composite(f) => {
                    let layout = space.layout();
                    let own_bits = match slot.indexable_field.as_ref() {
                        Some(fname) => {
                            let (o, w) = layout
                                .field_offset_width(fname)
                                .expect("composite own_field must exist");
                            id.extract_bits(o, w)
                        }
                        None => 0,
                    };
                    let shared: Vec<u64> = slot
                        .composite_dep_fields
                        .iter()
                        .map(|fname| {
                            let (o, w) = layout
                                .field_offset_width(fname)
                                .expect("composite shared field must exist");
                            id.extract_bits(o, w)
                        })
                        .collect();
                    values.insert(name.clone(), f(own_bits, &shared));
                }
                EvalFn::CrossSpaceStatic(f) => {
                    values.insert(name.clone(), f(id, self));
                }
                EvalFn::CrossSpaceTemporal(f) => match t {
                    Some(t) => {
                        values.insert(name.clone(), f(id, t, self));
                    }
                    None => {
                        errors.push((name.clone(), SpaceError::MissingTime { name: name.clone() }))
                    }
                },
            }
        }

        Ok(crate::space::EntitySnapshot::new(id, values, errors))
    }

    /// Register a global time function — a pure function of `t` only.
    pub fn register_global<V, F>(&mut self, name: &str, f: F) -> Result<(), WorldError>
    where
        V: Any + Send + Sync + 'static,
        F: Fn(DateTime<Utc>) -> V + Send + Sync + 'static,
    {
        if self.globals.contains_key(name) {
            return Err(WorldError::DuplicateGlobalName(name.to_string()));
        }
        let boxed: Arc<dyn Fn(DateTime<Utc>) -> Box<dyn Any + Send + Sync> + Send + Sync> =
            Arc::new(move |t| Box::new(f(t)));
        self.globals.insert(
            name.to_string(),
            GlobalSlot {
                value_type_id: TypeId::of::<V>(),
                value_type_name: type_label::<V>(),
                eval: boxed,
            },
        );
        Ok(())
    }

    /// Query a global time function's value.
    pub fn global<V: Any + 'static>(&self, name: &str, t: DateTime<Utc>) -> Result<V, WorldError> {
        let slot = self
            .globals
            .get(name)
            .ok_or_else(|| WorldError::UnknownGlobal(name.to_string()))?;

        if slot.value_type_id != TypeId::of::<V>() {
            return Err(WorldError::GlobalTypeMismatch {
                name: name.to_string(),
                expected: type_label::<V>(),
                actual: slot.value_type_name,
            });
        }

        let boxed = (slot.eval)(t);
        boxed
            .downcast::<V>()
            .map(|b| *b)
            .map_err(|_| WorldError::GlobalTypeMismatch {
                name: name.to_string(),
                expected: type_label::<V>(),
                actual: slot.value_type_name,
            })
    }

    pub fn global_names(&self) -> impl Iterator<Item = String> + '_ {
        self.globals.keys().cloned()
    }

    /// Enumerate the members of a relation registered on a named space.
    /// Thin pass-through; the underlying space's `related` does the work.
    pub fn related(
        &self,
        space: &str,
        owner_id: W,
        name: &str,
        t: DateTime<Utc>,
    ) -> Result<RelatedIter<'_, W>, WorldError> {
        let space_ref = self.space(space)?;
        Ok(space_ref.related(owner_id, name, t)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitLayout;
    use chrono::TimeZone;

    fn simple_space(name: &str) -> Space<u64> {
        let layout = BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        Space::<u64>::new(name, layout)
    }

    #[test]
    fn empty_world() {
        let world: World<u64> = World::new();
        assert!(!world.has_space("any"));
        assert_eq!(world.space_names().count(), 0);
    }

    #[test]
    fn register_and_lookup_space() {
        let mut world: World<u64> = World::new();
        world.register(simple_space("people")).unwrap();
        assert!(world.has_space("people"));
        assert_eq!(world.space("people").unwrap().name(), "people");
    }

    #[test]
    fn duplicate_space_name_rejected() {
        let mut world: World<u64> = World::new();
        world.register(simple_space("people")).unwrap();
        let err = world.register(simple_space("people")).unwrap_err();
        assert!(matches!(err, WorldError::DuplicateSpaceName(ref n) if n == "people"));
    }

    #[test]
    fn world_default_epoch_matches_canonical() {
        let world: World<u64> = World::new();
        assert_eq!(world.epoch(), crate::default_epoch());
    }

    #[test]
    fn world_with_epoch_overrides_default() {
        let pinned = chrono::Utc.with_ymd_and_hms(2087, 3, 15, 0, 0, 0).unwrap();
        let world: World<u64> = World::with_epoch(pinned);
        assert_eq!(world.epoch(), pinned);
    }

    #[test]
    fn world_set_epoch_mutates_in_place() {
        let mut world: World<u64> = World::new();
        let pinned = chrono::Utc.with_ymd_and_hms(1995, 6, 1, 0, 0, 0).unwrap();
        world.set_epoch(pinned);
        assert_eq!(world.epoch(), pinned);
    }

    #[test]
    fn unknown_space_lookup_errors() {
        let world: World<u64> = World::new();
        let err = world.space("nope").err().unwrap();
        assert!(matches!(err, WorldError::UnknownSpace(ref n) if n == "nope"));
    }

    #[test]
    fn space_names_iterates_all() {
        let mut world: World<u64> = World::new();
        world.register(simple_space("a")).unwrap();
        world.register(simple_space("b")).unwrap();
        let mut names: Vec<_> = world.space_names().collect();
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn world_routes_static_attribute() {
        let mut world: World<u64> = World::new();
        let mut space = simple_space("p");
        space
            .attribute::<f64, _>("score", |id: u64| (id as f64) * 0.01)
            .unwrap();
        world.register(space).unwrap();

        let v: f64 = world.attribute_value("p", 42u64, "score", None).unwrap();
        assert!((v - 0.42).abs() < 1e-9);
    }

    #[test]
    fn world_routes_temporal_attribute() {
        use chrono::{Datelike, TimeZone};
        let mut world: World<u64> = World::new();
        let mut space = simple_space("p");
        space
            .temporal_attribute::<i32, _>("year", |_, t: DateTime<Utc>| t.year())
            .unwrap();
        world.register(space).unwrap();

        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let y: i32 = world.attribute_value("p", 42u64, "year", Some(t)).unwrap();
        assert_eq!(y, 2026);
    }

    #[test]
    fn world_routes_cross_space_attribute() {
        use crate::space::Space;
        // people.score = families.score_for(person.family_id)
        let people_layout = BitLayout::<u64>::new(vec![("family_id", 16), ("rest", 48)]).unwrap();
        let families_layout = BitLayout::<u64>::new(vec![("fam", 16), ("rest", 48)]).unwrap();

        let mut people = Space::<u64>::new("people", people_layout.clone());
        let mut families = Space::<u64>::new("families", families_layout);

        // In families: a static attribute
        families
            .attribute::<f64, _>("score", |fam_id: u64| (fam_id & 0xFFFF) as f64 * 0.5)
            .unwrap();

        // In people: a cross-space attribute that reads family score
        people
            .indexable_attribute::<u64, _>("family_id", "family_id", |b| b)
            .unwrap();
        let layout_for_closure = people_layout.clone();
        people
            .cross_space_attribute::<f64, _>("family_score", move |id: u64, world: &World<u64>| {
                let fam_id = layout_for_closure.extract(id, "family_id");
                world
                    .attribute_value::<f64>("families", fam_id, "score", None)
                    .unwrap_or(0.0)
            })
            .unwrap();

        let mut world: World<u64> = World::new();
        world.register(people).unwrap();
        world.register(families).unwrap();

        // Build a person id with family_id = 7
        let alice = people_layout.compose(&[("family_id", 7)]);
        let score: f64 = world
            .attribute_value("people", alice, "family_score", None)
            .unwrap();
        assert!((score - 3.5).abs() < 1e-9);
    }

    #[test]
    fn world_unknown_space_errors() {
        let world: World<u64> = World::new();
        let err = world
            .attribute_value::<f64>("nope", 0u64, "any", None)
            .unwrap_err();
        assert!(matches!(err, WorldError::UnknownSpace(ref n) if n == "nope"));
    }

    #[test]
    fn world_propagates_space_error() {
        let mut world: World<u64> = World::new();
        world.register(simple_space("p")).unwrap();
        let err = world
            .attribute_value::<f64>("p", 0u64, "missing", None)
            .unwrap_err();
        assert!(matches!(err, WorldError::SpaceError(_)));
    }

    #[test]
    fn world_entity_snapshot() {
        let mut world: World<u64> = World::new();
        let mut space = simple_space("p");
        space.attribute::<f64, _>("score", |_| 0.5).unwrap();
        world.register(space).unwrap();
        let snap = world.entity("p", 0u64, None).unwrap();
        assert_eq!(*snap.get::<f64>("score").unwrap(), 0.5);
    }

    #[test]
    fn register_and_query_global() {
        use chrono::TimeZone;
        let mut world: World<u64> = World::new();
        world
            .register_global::<f64, _>("economy", |t: DateTime<Utc>| (t.timestamp() as f64).sin())
            .unwrap();
        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let v: f64 = world.global("economy", t).unwrap();
        // Deterministic check: calling twice gives the same result.
        assert_eq!(v, world.global::<f64>("economy", t).unwrap());
    }

    #[test]
    fn duplicate_global_rejected() {
        let mut world: World<u64> = World::new();
        world.register_global::<f64, _>("x", |_| 0.0).unwrap();
        let err = world.register_global::<f64, _>("x", |_| 1.0).unwrap_err();
        assert!(matches!(err, WorldError::DuplicateGlobalName(ref n) if n == "x"));
    }

    #[test]
    fn unknown_global_errors() {
        use chrono::TimeZone;
        let world: World<u64> = World::new();
        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let err = world.global::<f64>("missing", t).unwrap_err();
        assert!(matches!(err, WorldError::UnknownGlobal(ref n) if n == "missing"));
    }

    #[test]
    fn global_type_mismatch_rejected() {
        use chrono::TimeZone;
        let mut world: World<u64> = World::new();
        world.register_global::<f64, _>("x", |_| 0.5).unwrap();
        let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
        let err = world.global::<i32>("x", t).unwrap_err();
        assert!(matches!(err, WorldError::GlobalTypeMismatch { .. }));
    }

    #[test]
    fn global_names_iterates() {
        let mut world: World<u64> = World::new();
        world.register_global::<f64, _>("a", |_| 0.0).unwrap();
        world.register_global::<f64, _>("b", |_| 0.0).unwrap();
        let mut names: Vec<_> = world.global_names().collect();
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn world_related_enumerates_fixed_arity_across_spaces() {
        use crate::space::arity::Arity;
        use chrono::TimeZone;

        let layout = crate::bits::BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let mut families = Space::<u64>::new("families", layout);
        families
            .relation("parents", Arity::Fixed(2), |owner, _t, idx| {
                owner ^ (idx as u64 + 1)
            })
            .unwrap();

        let mut world: World<u64> = World::new();
        world.register(families).unwrap();

        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        let ids: Vec<u64> = world
            .related("families", 100u64, "parents", t)
            .unwrap()
            .collect();
        assert_eq!(ids, vec![100u64 ^ 1, 100u64 ^ 2]);
    }

    #[test]
    fn world_related_unknown_space_rejected() {
        use chrono::TimeZone;
        let world: World<u64> = World::new();
        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        let err = world.related("nope", 0u64, "parents", t).unwrap_err();
        assert!(matches!(err, WorldError::UnknownSpace(ref n) if n == "nope"));
    }

    #[test]
    fn world_related_unknown_relation_rejected() {
        use chrono::TimeZone;
        let layout = crate::bits::BitLayout::<u64>::new(vec![("x", 8)]).unwrap();
        let families = Space::<u64>::new("families", layout);
        let mut world: World<u64> = World::new();
        world.register(families).unwrap();

        let t = Utc.with_ymd_and_hms(2026, 4, 22, 0, 0, 0).unwrap();
        let err = world.related("families", 0u64, "missing", t).unwrap_err();
        // SpaceError::UnknownRelation is wrapped in WorldError::SpaceError
        match err {
            WorldError::SpaceError(inner) => {
                assert!(matches!(*inner, SpaceError::UnknownRelation(ref n) if n == "missing"));
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
