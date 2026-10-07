//! Embeds every world pack under `worlds/` (the repository root) into the
//! binary, so `Params::embedded("us")` never depends on the working
//! directory. Directory packs load at run time with `Params::load`.

use std::fs;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("readable worlds/")
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let worlds = manifest.parent().unwrap().join("worlds");
    println!("cargo:rerun-if-changed={}", worlds.display());
    let mut files = Vec::new();
    for pack in fs::read_dir(&worlds)
        .expect("a worlds/ directory")
        .flatten()
    {
        if pack.path().is_dir() {
            walk(&pack.path(), &mut files);
        }
    }
    files.sort();
    // A directory's timestamp changes only when entries come or go, so
    // every file is watched too (an edited pack must re-embed).
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        if let Some(d) = f.parent() {
            println!("cargo:rerun-if-changed={}", d.display());
        }
    }
    let mut code = String::from("&[\n");
    for f in &files {
        let rel = f.strip_prefix(&worlds).unwrap();
        let mut parts = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        let pack = parts.next().unwrap();
        let path: Vec<String> = parts.collect();
        code += &format!(
            "    ::internot_def::EmbeddedFile {{ pack: {:?}, path: {:?}, bytes: include_bytes!({:?}) }},\n",
            pack,
            path.join("/"),
            f.display().to_string()
        );
    }
    code += "]\n";
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded_packs.rs");
    fs::write(out, code).unwrap();
}
