//! The society world (`internot_society`): people, kinship, households,
//! names and residence as pure functions of `(seed, id, t)`. One per pack
//! per process, built on first use and shared by every `Universe`.
//!
//! The pack is `INTERNOT_PACK` (default `us`; `us-areas` is the area-mode
//! world, much larger to build), the seed `INTERNOT_SEED` (default 42).
//! Spec: `docs/superpowers/specs/2026-10-01-directory.md`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use internot_society::residence::{Places, Residence};
use internot_society::{Params, World};

/// The default pack and seed.
pub const DEFAULT_PACK: &str = "us";
pub const DEFAULT_SEED: u64 = 42;

/// A built society world with its residence.
pub struct Society {
    pub pack: String,
    pub seed: u64,
    pub world: World,
    pub residence: Residence,
}

impl Society {
    /// The pack and seed named by `INTERNOT_PACK` and `INTERNOT_SEED`.
    pub fn from_env() -> &'static Society {
        let pack = std::env::var("INTERNOT_PACK").unwrap_or_else(|_| DEFAULT_PACK.to_string());
        let seed = std::env::var("INTERNOT_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_SEED);
        Self::shared(&pack, seed)
    }

    /// The world of embedded pack `pack` and `seed`, built once per process.
    pub fn shared(pack: &str, seed: u64) -> &'static Society {
        static BUILT: OnceLock<Mutex<HashMap<(String, u64), &'static Society>>> = OnceLock::new();
        let mut built = BUILT
            .get_or_init(Default::default)
            .lock()
            .expect("the society cache is not poisoned");
        built
            .entry((pack.to_string(), seed))
            .or_insert_with(|| Box::leak(Box::new(Self::build(pack, seed))))
    }

    fn build(pack: &str, seed: u64) -> Society {
        let params =
            Params::embedded(pack).unwrap_or_else(|e| panic!("world pack `{pack}`: {e}"));
        let world = World::build(params, seed);
        let places = Places::from_params(&world.ledger().params)
            .unwrap_or_else(|e| panic!("world pack `{pack}` places: {e}"));
        let residence = Residence::build(&world, places);
        Society {
            pack: pack.to_string(),
            seed,
            world,
            residence,
        }
    }
}
