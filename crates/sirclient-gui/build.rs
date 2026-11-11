//! Embeds every `locales/*.ftl` file into the binary, so adding a language
//! only needs a new file (see docs/TRANSLATING.md).

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("locales");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("locales directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
        .collect();
    files.sort();
    let mut out = String::from("pub static EMBEDDED: &[(&str, &str)] = &[\n");
    for f in files {
        println!("cargo:rerun-if-changed={}", f.display());
        let id = f.file_stem().unwrap().to_string_lossy();
        out.push_str(&format!(
            "    ({id:?}, include_str!({:?})),\n",
            f.display().to_string()
        ));
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("locales.rs");
    std::fs::write(dest, out).unwrap();
}
