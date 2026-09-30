//! `Tie` — an edge on the social graph at a point in time.
//!
//! Domain-free: `kind` is a `u8` encoding whose meaning is defined by
//! the consumer (e.g. `internot::social::HouseholdRole`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Sentinel value for `Tie::kind` when no specific role is known.
pub const TIE_KIND_UNSPECIFIED: u8 = 0;

/// One edge of the social graph at time `t`. Reciprocity is structural:
/// if A has a Tie pointing at B, B has a Tie pointing at A with the
/// same `venue_id`, the same `strength` (modulo floating-point), and
/// a `kind` that's the symmetric counterpart in the consumer's role
/// encoding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tie {
    /// Mail-id of the other endpoint.
    pub peer_id: u32,
    /// Consumer-encoded tie role (e.g. `HouseholdRole::Partner as u8`).
    /// `TIE_KIND_UNSPECIFIED` (0) when no specific role applies.
    pub kind: u8,
    /// Venue (or pair-key) the tie is anchored on.
    pub venue_id: u64,
    /// Procedural strength at time `since` — `[0.0, 1.0]`.
    pub strength: f64,
    /// When the tie began (cohabit start, marriage date, etc.).
    pub since: DateTime<Utc>,
    /// Most recent procedural contact event. Sessions may override
    /// this with overlay writes to model agent actions.
    pub last_proc_contact: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn tie_roundtrips_json() {
        let t = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let tie = Tie {
            peer_id: 42,
            kind: 3,
            venue_id: 0xdeadbeef_u64,
            strength: 0.75,
            since: t,
            last_proc_contact: t,
        };
        let s = serde_json::to_string(&tie).unwrap();
        let back: Tie = serde_json::from_str(&s).unwrap();
        assert_eq!(tie, back);
    }
}
