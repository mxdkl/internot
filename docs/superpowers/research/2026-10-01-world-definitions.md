# World definitions: configurable, extensible worlds

**Date:** 2026-10-01
**Why:** the founder, 2026-10-01: "I want everything configurable through those rust json-like files. Everything means defining races, names, birth rates, countries, places (the 'everything is statistics' I said earlier), and later services. It should be easily extendible by anyone. You can decide the specifics."

## 1. The problem's usual name

This is **data-driven design** and **moddability**: a simulation's mechanisms live in code, and every content and parameter value lives in data files that users can edit, extend and layer. It is well-trodden in games and simulators:

| System | Data format | Inheritance and overrides | Lesson |
|---|---|---|---|
| **RimWorld** "Defs" | XML | A def names a `ParentName` (inherit and override fields); `Abstract` defs exist only to be inherited. Mods ship `PatchOperation`s that edit other mods' defs by path. | Inheritance plus path patches let thousands of independent mods coexist. |
| **Paradox** (Clausewitz) | Custom script | Mods replace whole files or add new ones; later load order wins. | Whole-file replacement is simple but makes small tweaks clumsy, since a mod copies the whole file to change one number. |
| **Factorio** | Lua `data` stage | Mods run in order and edit the shared prototype table; the engine freezes it before play. | Separating a data stage (merge everything, validate) from runtime (fast, immutable) keeps the game loop simple. |
| **Bevy** ecosystem | RON assets | serde-typed assets loaded from files. | RON maps directly onto Rust types (structs, enums, tuples), with comments. |
| **Cargo**, many Rust tools | TOML | No inheritance (workspace inheritance only). | TOML is excellent for flat settings, awkward for nested tables of tuples. |

## 2. Format choice

| Format | Fit |
|---|---|
| **RON** (Rusty Object Notation) | Rust's JSON-like format: structs `(a: 1)`, enums `Variant(...)`, tuples, maps, lists, comments, trailing commas. serde-native (`ron` crate). Typed and readable. |
| TOML | Good for flat settings. Arrays of anchor tuples (`[[1840, 0.004], ...]`) lose their meaning. No enums. |
| JSON / JSON5 | No comments (JSON); weaker Rust mapping; everybody knows it. |
| YAML | Indentation pitfalls and implicit typing (the "Norway problem"). |
| KDL | Nice, but a small ecosystem. |

**RON** is what the founder named ("rust json-like files"). It holds this model's vocabulary naturally: enums for schedule kinds, tuples for `(year, value)` anchors, and comments next to each number for its source.

## 3. What has to hold for this project

1. **Determinism.** A world is a pure function of `(definition, seed)`.
   - Parsing must be exact. `ron` reads a float token and parses it with Rust's correctly rounded `f64::from_str`, so `0.0110` in a file is bit-identical to the literal `0.0110` in code.
   - Every derived quantity keeps the same arithmetic (the migration is checked by `examples/world_fingerprint.rs`).
2. **Speed.** Hot paths (death draws at about 200 ns) must not pay for flexibility.
   - Factorio's split applies: load, merge, validate and **compile** the definition into immutable tables once, at `World::build`. Lookups read the compiled form.
3. **Safety for non-experts.**
   - Unknown fields are errors (`#[serde(deny_unknown_fields)]`), so typos don't silently fall back to defaults.
   - Every validation error names the file and the field.
   - Shares must lie in [0, 1], anchors must be in year order, distributions must sum to 1 within a tolerance, ages must fit the code's structural bounds, and group ids must resolve.
4. **Extension by anyone.**
   - A pack can `extend` another and override only what it changes (RimWorld-style inheritance, at field granularity), so "the US world without immigration" is one small file.
   - New things (a heritage group, a region, later a service) are entries in lists, keyed by string ids, never Rust enums.
5. **Sources next to numbers.** Today each number's citation is in a Rust doc comment. In packs, the comment moves into the `.ron` file beside the number.
6. **Identity.** Caches (LLM renders) and published seeds need to know which definition built a world. A fingerprint of the merged definition (canonical serialization) plus its data files' hashes identifies it.

## 4. What stays in code

Mechanisms, and the structural bounds they need:
- the ledger;
- the markets and their IPF;
- the functional forms: Siler mortality, the log-logistic first-union timing, the Rogers–Castro migration age profile;
- keyed draws;
- array sizes (maximum age, dissolution classes, union chains).

A pack can choose parameters and, where the code offers alternatives, a form (an enum). It can't add new mechanisms; that is code. Statistical values that exist only because of a mechanism (for example the IPF tolerance) stay in code.
