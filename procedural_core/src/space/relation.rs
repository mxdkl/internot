//! Relation declarations and enumeration machinery.

use crate::space::arity::Arity;
use crate::word::BitWord;
use chrono::{DateTime, Utc};
use std::fmt;
use std::sync::Arc;

pub(crate) struct RelationSlot<W: BitWord> {
    pub(crate) arity: Arity<W>,
    pub(crate) member_derive: Arc<dyn Fn(W, DateTime<Utc>, usize) -> W + Send + Sync>,
}

impl<W: BitWord> fmt::Debug for RelationSlot<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RelationSlot")
            .field("arity", &self.arity)
            .field("member_derive", &"<closure>")
            .finish()
    }
}

#[derive(Debug)]
pub struct RelatedIter<'a, W: BitWord> {
    slot: &'a RelationSlot<W>,
    owner_id: W,
    t: DateTime<Utc>,
    count: usize,
    index: usize,
}

impl<'a, W: BitWord> RelatedIter<'a, W> {
    pub(crate) fn new(slot: &'a RelationSlot<W>, owner_id: W, t: DateTime<Utc>) -> Self {
        let count = slot.arity.resolve(owner_id, t);
        RelatedIter {
            slot,
            owner_id,
            t,
            count,
            index: 0,
        }
    }
}

impl<'a, W: BitWord> Iterator for RelatedIter<'a, W> {
    type Item = W;

    fn next(&mut self) -> Option<W> {
        if self.index >= self.count {
            return None;
        }
        let v = (self.slot.member_derive)(self.owner_id, self.t, self.index);
        self.index += 1;
        Some(v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, W: BitWord> ExactSizeIterator for RelatedIter<'a, W> {}
