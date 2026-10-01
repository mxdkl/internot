//! Loading packs: sources, `extends`, the merge, and the fingerprint.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ron::Value;
use serde::de::DeserializeOwned;

use crate::DefError;

/// Where packs come from.
pub trait Source: Sync {
    /// Every file of `pack` (paths relative to the pack, `/`-separated), or
    /// `None` if this source doesn't have the pack.
    fn list(&self, pack: &str) -> Option<Vec<String>>;
    /// The bytes of `path` in `pack`.
    fn read(&self, pack: &str, path: &str) -> Option<Cow<'static, [u8]>>;
}

/// One file of a pack compiled into the binary.
#[derive(Clone, Copy, Debug)]
pub struct EmbeddedFile {
    pub pack: &'static str,
    pub path: &'static str,
    pub bytes: &'static [u8],
}

/// Packs compiled into the binary (`include_bytes!`), so loading them never
/// depends on the working directory.
#[derive(Clone, Copy, Debug)]
pub struct Embedded(pub &'static [EmbeddedFile]);

impl Source for Embedded {
    fn list(&self, pack: &str) -> Option<Vec<String>> {
        let files: Vec<String> = self
            .0
            .iter()
            .filter(|f| f.pack == pack)
            .map(|f| f.path.to_string())
            .collect();
        (!files.is_empty()).then_some(files)
    }

    fn read(&self, pack: &str, path: &str) -> Option<Cow<'static, [u8]>> {
        self.0
            .iter()
            .find(|f| f.pack == pack && f.path == path)
            .map(|f| Cow::Borrowed(f.bytes))
    }
}

/// Packs as directories under a root: pack `name` is `root/name/`.
#[derive(Clone, Debug)]
pub struct Dir(pub PathBuf);

impl Source for Dir {
    fn list(&self, pack: &str) -> Option<Vec<String>> {
        let root = self.0.join(pack);
        if !root.is_dir() {
            return None;
        }
        let mut out = Vec::new();
        walk(&root, &root, &mut out);
        out.sort();
        Some(out)
    }

    fn read(&self, pack: &str, path: &str) -> Option<Cow<'static, [u8]>> {
        std::fs::read(self.0.join(pack).join(path))
            .ok()
            .map(Cow::Owned)
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out);
        } else if let Ok(rel) = p.strip_prefix(root) {
            let rel: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(rel.join("/"));
        }
    }
}

/// A section: the merged value, and the text when a single file supplied
/// it (deserializing the text gives errors with line numbers).
#[derive(Clone, Debug)]
struct Section {
    value: Value,
    text: Option<String>,
    /// `pack/path` of every file merged into it, parent first.
    files: Vec<String>,
}

/// A loaded, merged world pack.
#[derive(Clone, Debug)]
pub struct Pack {
    name: String,
    /// The `extends` chain, root first, ending with `name`.
    chain: Vec<String>,
    sections: BTreeMap<String, Section>,
    data: BTreeMap<String, Cow<'static, [u8]>>,
    fingerprint: u64,
}

impl Pack {
    /// Load pack `name` from the first source that has it, following
    /// `extends` in `world.ron` (each parent from the first source that has
    /// it, so a directory pack can extend an embedded one).
    pub fn load(name: &str, sources: &[&dyn Source]) -> Result<Pack, DefError> {
        let mut seen = BTreeSet::new();
        let mut pack = Self::load_chain(name, sources, &mut seen)?;
        pack.fingerprint = pack.compute_fingerprint();
        Ok(pack)
    }

    fn load_chain(
        name: &str,
        sources: &[&dyn Source],
        seen: &mut BTreeSet<String>,
    ) -> Result<Pack, DefError> {
        if !seen.insert(name.to_string()) {
            return Err(DefError::new(
                name,
                Some("world.ron"),
                None,
                "`extends` forms a cycle".into(),
            ));
        }
        let (source, files) = sources
            .iter()
            .find_map(|s| s.list(name).map(|f| (*s, f)))
            .ok_or_else(|| DefError::new(name, None, None, "no such pack in any source".into()))?;
        let read = |path: &str| {
            source
                .read(name, path)
                .ok_or_else(|| DefError::new(name, Some(path), None, "cannot read the file".into()))
        };
        // This pack's own sections and data.
        let mut own: BTreeMap<String, (Value, String)> = BTreeMap::new();
        let mut data = BTreeMap::new();
        for path in &files {
            let bytes = read(path)?;
            match path.strip_suffix(".ron") {
                Some(section) => {
                    let text = String::from_utf8(bytes.into_owned()).map_err(|_| {
                        DefError::new(name, Some(path), None, "not valid UTF-8".into())
                    })?;
                    let value: Value = ron::from_str(&text)
                        .map_err(|e| DefError::new(name, Some(path), None, e.to_string()))?;
                    own.insert(section.to_string(), (value, text));
                }
                None => {
                    data.insert(path.clone(), bytes);
                }
            }
        }
        let world = own.get("world").ok_or_else(|| {
            DefError::new(
                name,
                Some("world.ron"),
                None,
                "every pack needs a world.ron".into(),
            )
        })?;
        let parent = match field(&world.0, "extends") {
            None | Some(Value::Unit) | Some(Value::Option(None)) => None,
            Some(Value::Option(Some(v))) => Some(as_string(v, name)?),
            Some(v) => Some(as_string(v, name)?),
        };

        let mut pack = match parent {
            Some(p) => Self::load_chain(&p, sources, seen)?,
            None => Pack {
                name: String::new(),
                chain: Vec::new(),
                sections: BTreeMap::new(),
                data: BTreeMap::new(),
                fingerprint: 0,
            },
        };
        pack.name = name.to_string();
        pack.chain.push(name.to_string());
        for (section, (value, text)) in own {
            let file = format!("{name}/{section}.ron");
            match pack.sections.get_mut(&section) {
                Some(s) => {
                    merge(&mut s.value, value);
                    s.text = None;
                    s.files.push(file);
                }
                None => {
                    pack.sections.insert(
                        section,
                        Section {
                            value,
                            text: Some(text),
                            files: vec![file],
                        },
                    );
                }
            }
        }
        pack.data.extend(data);
        Ok(pack)
    }

    /// The pack's name (the last in its `extends` chain).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The `extends` chain, root first.
    pub fn chain(&self) -> &[String] {
        &self.chain
    }

    /// Whether the pack (or a parent) has section `name`.
    pub fn has_section(&self, name: &str) -> bool {
        self.sections.contains_key(name)
    }

    /// Section `name` (the file `name.ron`, merged along the chain) as `T`.
    pub fn section<T: DeserializeOwned>(&self, name: &str) -> Result<T, DefError> {
        let file = format!("{name}.ron");
        let s = self.sections.get(name).ok_or_else(|| {
            DefError::new(&self.name, Some(&file), None, "missing section".into())
        })?;
        let origin = s.files.join(" + ");
        match &s.text {
            // One file: errors carry its line and column.
            Some(text) => ron::from_str(text)
                .map_err(|e| DefError::new(&self.name, Some(&origin), None, e.to_string())),
            // Merged: errors carry the field path.
            None => serde_path_to_error::deserialize(s.value.clone()).map_err(|e| {
                DefError::new(
                    &self.name,
                    Some(&origin),
                    Some(e.path().to_string()),
                    e.inner().to_string(),
                )
            }),
        }
    }

    /// The bytes of data file `path` (e.g. `data/names.bin`).
    pub fn data(&self, path: &str) -> Result<&[u8], DefError> {
        self.data
            .get(path)
            .map(|b| b.as_ref())
            .ok_or_else(|| DefError::new(&self.name, Some(path), None, "missing data file".into()))
    }

    /// A 64-bit fingerprint of the merged sections and the data files: two
    /// packs with equal fingerprints define the same worlds.
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn compute_fingerprint(&self) -> u64 {
        let mut h = xxhash_rust::xxh3::Xxh3::new();
        for (name, s) in &self.sections {
            h.update(name.as_bytes());
            h.update(&[0]);
            canonical(&s.value, &mut h);
        }
        for (path, bytes) in &self.data {
            h.update(path.as_bytes());
            h.update(&[0]);
            h.update(&(bytes.len() as u64).to_le_bytes());
            h.update(bytes);
        }
        h.digest()
    }
}

fn field<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Map(m) => m.get(&Value::String(key.to_string())),
        _ => None,
    }
}

fn as_string(v: &Value, pack: &str) -> Result<String, DefError> {
    match v {
        Value::String(s) => Ok(s.clone()),
        _ => Err(DefError::new(
            pack,
            Some("world.ron"),
            Some("extends".into()),
            "expected a pack name, e.g. `extends: Some(\"us\")`".into(),
        )),
    }
}

/// Merge `over` into `base`, the child winning:
/// - maps (records) merge key by key, recursively;
/// - lists whose items are all records with an `id` merge by id: a matching
///   id merges its fields, a new id is appended;
/// - `Some(x)` into `Some(y)` merges `x` into `y`;
/// - anything else replaces.
pub fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Map(b), Value::Map(o)) => {
            for (k, v) in o.into_iter() {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (Value::Seq(b), Value::Seq(o)) if keyed(b) && keyed(&o) && !o.is_empty() => {
            for item in o {
                let id = id_of(&item).cloned();
                match b.iter_mut().find(|x| id_of(x) == id.as_ref()) {
                    Some(slot) => merge(slot, item),
                    None => b.push(item),
                }
            }
        }
        (Value::Option(Some(b)), Value::Option(Some(o))) => merge(b, *o),
        (b, o) => *b = o,
    }
}

fn id_of(v: &Value) -> Option<&Value> {
    field(v, "id")
}

fn keyed(items: &[Value]) -> bool {
    items.iter().all(|x| id_of(x).is_some())
}

/// Hash a value canonically: map entries in key order, numbers by value.
fn canonical(v: &Value, h: &mut xxhash_rust::xxh3::Xxh3) {
    match v {
        Value::Bool(b) => h.update(&[1, *b as u8]),
        Value::Char(c) => {
            h.update(&[2]);
            h.update(&(*c as u32).to_le_bytes());
        }
        Value::Map(m) => {
            h.update(&[3]);
            let mut entries: Vec<(String, &Value)> = m
                .iter()
                .map(|(k, v)| (ron::to_string(k).unwrap_or_default(), v))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            h.update(&(entries.len() as u64).to_le_bytes());
            for (k, v) in entries {
                h.update(k.as_bytes());
                h.update(&[0]);
                canonical(v, h);
            }
        }
        Value::Number(n) => {
            h.update(&[4]);
            h.update(&n.into_f64().to_bits().to_le_bytes());
        }
        Value::Option(o) => {
            h.update(&[5]);
            if let Some(x) = o {
                canonical(x, h);
            }
        }
        Value::String(s) => {
            h.update(&[6]);
            h.update(s.as_bytes());
            h.update(&[0]);
        }
        Value::Bytes(b) => {
            h.update(&[7]);
            h.update(b);
        }
        Value::Seq(items) => {
            h.update(&[8]);
            h.update(&(items.len() as u64).to_le_bytes());
            for x in items {
                canonical(x, h);
            }
        }
        Value::Unit => h.update(&[9]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    static FILES: &[EmbeddedFile] = &[
        EmbeddedFile {
            pack: "base",
            path: "world.ron",
            bytes: b"(name: \"Base\", extends: None, regions: [(id: \"a\", w: 1.0), (id: \"b\", w: 2.0)])",
        },
        EmbeddedFile {
            pack: "base",
            path: "rates.ron",
            bytes: b"// A comment.\n(level: 0.5, anchors: [(1900, 1.0), (2000, 2.0)], nested: (x: 1, y: 2))",
        },
        EmbeddedFile {
            pack: "base",
            path: "data/blob.bin",
            bytes: &[1, 2, 3],
        },
        EmbeddedFile {
            pack: "child",
            path: "world.ron",
            bytes: b"(name: \"Child\", extends: Some(\"base\"), regions: [(id: \"b\", w: 5.0), (id: \"c\", w: 1.0)])",
        },
        EmbeddedFile {
            pack: "child",
            path: "rates.ron",
            bytes: b"(nested: (y: 3), anchors: [(1950, 9.0)])",
        },
        EmbeddedFile {
            pack: "loop1",
            path: "world.ron",
            bytes: b"(name: \"L\", extends: Some(\"loop2\"), regions: [])",
        },
        EmbeddedFile {
            pack: "loop2",
            path: "world.ron",
            bytes: b"(name: \"L\", extends: Some(\"loop1\"), regions: [])",
        },
    ];

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct World {
        name: String,
        extends: Option<String>,
        regions: Vec<Region>,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Region {
        id: String,
        w: f64,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Rates {
        level: f64,
        anchors: Vec<(i32, f64)>,
        nested: Nested,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Nested {
        x: i32,
        y: i32,
    }

    fn load(name: &str) -> Result<Pack, DefError> {
        Pack::load(name, &[&Embedded(FILES)])
    }

    #[test]
    fn a_child_overrides_only_what_it_names() {
        let p = load("child").unwrap();
        assert_eq!(p.chain(), ["base", "child"]);
        let w: World = p.section("world").unwrap();
        assert_eq!(w.name, "Child");
        // Records with ids merge by id; new ids are appended.
        let ids: Vec<(&str, f64)> = w.regions.iter().map(|r| (r.id.as_str(), r.w)).collect();
        assert_eq!(ids, [("a", 1.0), ("b", 5.0), ("c", 1.0)]);
        let r: Rates = p.section("rates").unwrap();
        assert_eq!(r.level, 0.5, "inherited");
        assert_eq!(r.anchors, [(1950, 9.0)], "plain lists are replaced");
        assert_eq!(
            r.nested,
            Nested { x: 1, y: 3 },
            "records merge field by field"
        );
        assert_eq!(p.data("data/blob.bin").unwrap(), &[1, 2, 3]);
    }

    #[test]
    fn fingerprints_follow_content() {
        let (a, b) = (load("base").unwrap(), load("child").unwrap());
        assert_ne!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint(), load("base").unwrap().fingerprint());
    }

    #[test]
    fn errors_say_where() {
        assert!(load("loop1").unwrap_err().message.contains("cycle"));
        assert!(load("nowhere").is_err());
        // A typo in a single-file section: line and column.
        static TYPO: &[EmbeddedFile] = &[
            EmbeddedFile {
                pack: "t",
                path: "world.ron",
                bytes: b"(name: \"T\", extends: None, regions: [])",
            },
            EmbeddedFile {
                pack: "t",
                path: "rates.ron",
                bytes: b"(level: 0.5,\n anchors: [],\n nested: (x: 1, yy: 2))",
            },
        ];
        let p = Pack::load("t", &[&Embedded(TYPO)]).unwrap();
        let e = p.section::<Rates>("rates").unwrap_err();
        assert!(e.to_string().contains("3:"), "{e}");
        assert!(e.to_string().contains("yy"), "{e}");
        // In a merged section: the field path.
        static TYPO2: &[EmbeddedFile] = &[
            EmbeddedFile {
                pack: "t2",
                path: "world.ron",
                bytes: b"(name: \"T\", extends: Some(\"base\"), regions: [])",
            },
            EmbeddedFile {
                pack: "t2",
                path: "rates.ron",
                bytes: b"(nested: (x: \"one\"))",
            },
        ];
        let p = Pack::load("t2", &[&Embedded(TYPO2), &Embedded(FILES)]).unwrap();
        let e = p.section::<Rates>("rates").unwrap_err();
        assert_eq!(e.at.as_deref(), Some("nested.x"), "{e}");
    }
}
