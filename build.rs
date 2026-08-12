#![expect(
    clippy::unwrap_used,
    reason = "build script must fail immediately on invalid extracted block data"
)]

use std::{env, fs, path::Path, process::Command};

#[path = "build/blocks_codegen.rs"]
mod blocks_codegen;

fn write_generated(out_dir: &Path, name: &str, content: String) {
    let path = out_dir.join(name);
    let write = fs::read_to_string(&path)
        .ok()
        .is_none_or(|existing| existing != content);
    if write {
        fs::write(&path, content).unwrap();
        let _ = Command::new("rustfmt").arg(&path).output();
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let out_dir = Path::new(&manifest_dir).join("src/registry/generated");

    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).unwrap();
    }

    write_generated(
        &out_dir,
        "vanilla_blocks.rs",
        blocks_codegen::build().to_string(),
    );
}
