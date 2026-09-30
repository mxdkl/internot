//! Entity snapshot — bulk evaluation of all attributes at a fixed (id, t).

use crate::space::error::{type_label, SpaceError};
use crate::word::BitWord;
use std::any::Any;
use std::collections::HashMap;

pub struct EntitySnapshot<W: BitWord> {
    pub(crate) id: W,
    pub(crate) values: HashMap<String, Box<dyn Any + Send + Sync>>,
    pub(crate) errors: Vec<(String, SpaceError)>,
}

impl<W: BitWord> EntitySnapshot<W> {
    pub(crate) fn new(
        id: W,
        values: HashMap<String, Box<dyn Any + Send + Sync>>,
        errors: Vec<(String, SpaceError)>,
    ) -> Self {
        Self { id, values, errors }
    }

    pub fn id(&self) -> W {
        self.id
    }

    /// Borrow a typed attribute value from the snapshot. Type-checked via downcast.
    pub fn get<V: Any + 'static>(&self, name: &str) -> Result<&V, SpaceError> {
        let boxed = self
            .values
            .get(name)
            .ok_or_else(|| SpaceError::UnknownAttribute(name.to_string()))?;
        boxed
            .downcast_ref::<V>()
            .ok_or_else(|| SpaceError::AttributeTypeMismatch {
                name: name.to_string(),
                expected: type_label::<V>(),
                actual: "unknown",
            })
    }

    pub fn errors(&self) -> &[(String, SpaceError)] {
        &self.errors
    }

    pub fn attribute_names(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }
}
