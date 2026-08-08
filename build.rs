#![expect(
    clippy::unwrap_used,
    reason = "build script must fail immediately on invalid extracted block data"
)]

use std::{env, fs, path::Path, process::Command};

#[path = "build/blocks_codegen.rs"]
mod blocks_codegen;

fn main() {
    println!("cargo:rerun-if-changed=build/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let out_dir = Path::new(&manifest_dir).join("src/registry/generated");

    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).unwrap();
    }

    let path = out_dir.join("vanilla_blocks.rs");
    let content = blocks_codegen::build().to_string();

    let write = fs::read_to_string(&path)
        .ok()
        .is_none_or(|existing| existing != content);
    if write {
        fs::write(&path, content).unwrap();
        let _ = Command::new("rustfmt").arg(&path).output();
    }
}
