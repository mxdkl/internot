//! The society world (`internot_society::mono`): people and kinship as pure
//! functions of `(seed, id, t)`, with nothing stored per person. One per
//! pack, seed and scale per process, built on first use (milliseconds) and
//! shared by every `Universe`.
//!
//! The pack is `INTERNOT_PACK` (default `us`), the seed `INTERNOT_SEED`
//! (default 42), the scale `INTERNOT_SCALE` (default 1: the pack's own
//! population; the world's memory and lookups don't grow with it).
//! Spec: `docs/superpowers/specs/2026-10-01-directory.md`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use internot_society::mono::Mono;
use internot_society::Params;

/// The default pack, seed and scale.
pub const DEFAULT_PACK: &str = "us";
pub const DEFAULT_SEED: u64 = 42;
pub const DEFAULT_SCALE: f64 = 1.0;

/// A built society world.
pub struct Society {
    pub pack: String,
    pub seed: u64,
    pub scale: f64,
    pub world: Mono<'static>,
}

impl Society {
    /// The pack, seed and scale named by `INTERNOT_PACK`, `INTERNOT_SEED`
    /// and `INTERNOT_SCALE`.
    pub fn from_env() -> &'static Society {
        let pack = std::env::var("INTERNOT_PACK").unwrap_or_else(|_| DEFAULT_PACK.to_string());
        let seed = std::env::var("INTERNOT_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_SEED);
        let scale = std::env::var("INTERNOT_SCALE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_SCALE);
        Self::shared(&pack, seed, scale)
    }

    /// The world of embedded pack `pack`, `seed` and `scale`, built once per
    /// process.
    pub fn shared(pack: &str, seed: u64, scale: f64) -> &'static Society {
        static BUILT: OnceLock<Mutex<HashMap<(String, u64, u64), &'static Society>>> = OnceLock::new();
        let mut built = BUILT
            .get_or_init(Default::default)
            .lock()
            .expect("the society cache is not poisoned");
        built
            .entry((pack.to_string(), seed, scale.to_bits()))
            .or_insert_with(|| Box::leak(Box::new(Self::build(pack, seed, scale))))
    }

    fn build(pack: &str, seed: u64, scale: f64) -> Society {
        let params: &'static Params = Box::leak(Box::new(
            Params::embedded(pack).unwrap_or_else(|e| panic!("world pack `{pack}`: {e}")),
        ));
        Society {
            pack: pack.to_string(),
            seed,
            scale,
            world: Mono::new(params, seed, scale),
        }
    }
}
