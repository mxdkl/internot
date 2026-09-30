//! On-disk JSON cache keyed by (namespace, prompt_version,
//! model_version, hashed-id). The whole point of the cache is that
//! re-rendering the same AVM under the same prompt+model never costs
//! API time twice — the procedural id IS the cache key.

use std::path::PathBuf;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::RenderError;

#[derive(Debug, Clone)]
pub struct CacheKey {
    /// e.g. "mail_message", "calendar_event"
    pub namespace: String,
    /// Unique within namespace. For mail messages this is the
    /// 127-bit message_id rendered as hex.
    pub id: String,
    /// Bumped when the prompt template changes — invalidates cache.
    pub prompt_version: String,
    /// Pinned model SKU + date string. Different model = different cache.
    pub model_version: String,
}

#[derive(Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Look up a cached value. `None` = miss; `Some(Err(_))` = the
    /// file existed but couldn't be parsed (treated as miss by
    /// callers that want strict semantics).
    pub fn get<T: DeserializeOwned>(&self, key: &CacheKey) -> Option<T> {
        let path = self.path_for(key);
        let bytes = std::fs::read(&path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn put<T: Serialize>(&self, key: &CacheKey, value: &T) -> Result<(), RenderError> {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(RenderError::Encode)?;
        std::fs::write(&path, bytes)?;
        Ok(())
    }

    pub fn path_for(&self, key: &CacheKey) -> PathBuf {
        let h = format!("{:016x}", xxhash_rust::xxh3::xxh3_64(key.id.as_bytes()));
        self.root
            .join(&key.namespace)
            .join(&key.prompt_version)
            .join(&key.model_version)
            .join(format!("{h}.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Sample {
        a: u32,
        b: String,
    }

    #[test]
    fn round_trip_through_disk() {
        let dir = std::env::temp_dir().join(format!(
            "internot_renderer_cache_test_{}",
            std::process::id()
        ));
        let cache = Cache::at(&dir);
        let key = CacheKey {
            namespace: "test".into(),
            id: "id-1".into(),
            prompt_version: "v1".into(),
            model_version: "fake-model".into(),
        };
        let value = Sample { a: 42, b: "hello".into() };
        cache.put(&key, &value).unwrap();
        let got: Sample = cache.get(&key).expect("hit");
        assert_eq!(got, value);
        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn miss_returns_none() {
        let dir = std::env::temp_dir().join(format!(
            "internot_renderer_cache_miss_{}",
            std::process::id()
        ));
        let cache = Cache::at(&dir);
        let key = CacheKey {
            namespace: "test".into(),
            id: "nope".into(),
            prompt_version: "v1".into(),
            model_version: "fake-model".into(),
        };
        let got: Option<Sample> = cache.get(&key);
        assert!(got.is_none());
    }

    #[test]
    fn different_versions_isolate() {
        let dir = std::env::temp_dir().join(format!(
            "internot_renderer_cache_iso_{}",
            std::process::id()
        ));
        let cache = Cache::at(&dir);
        let key_v1 = CacheKey {
            namespace: "test".into(),
            id: "shared-id".into(),
            prompt_version: "v1".into(),
            model_version: "m".into(),
        };
        let key_v2 = CacheKey {
            namespace: "test".into(),
            id: "shared-id".into(),
            prompt_version: "v2".into(),
            model_version: "m".into(),
        };
        cache.put(&key_v1, &Sample { a: 1, b: "v1".into() }).unwrap();
        cache.put(&key_v2, &Sample { a: 2, b: "v2".into() }).unwrap();
        let got_v1: Sample = cache.get(&key_v1).unwrap();
        let got_v2: Sample = cache.get(&key_v2).unwrap();
        assert_eq!(got_v1.a, 1);
        assert_eq!(got_v2.a, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
