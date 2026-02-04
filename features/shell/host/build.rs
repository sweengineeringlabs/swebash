use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Locate engine.wasm: honour ENGINE_WASM env var, otherwise derive from
    // the target directory that Cargo uses for build artifacts.
    let wasm_src = if let Ok(p) = env::var("ENGINE_WASM") {
        PathBuf::from(p)
    } else {
        // OUT_DIR lives somewhere under <target-dir>/<profile>/build/<pkg>-<hash>/out.
        // Walk up from OUT_DIR until we find an ancestor that contains the
        // wasm32-unknown-unknown sub-tree.  This works regardless of whether
        // the target directory is the default `target/` or a custom path set
        // via CARGO_TARGET_DIR / .cargo/config.toml.
        let mut search = out_dir.clone();
        loop {
            let candidate = search
                .join("wasm32-unknown-unknown")
                .join("release")
                .join("engine.wasm");
            if candidate.exists() {
                break candidate;
            }
            if !search.pop() {
                panic!(
                    "build.rs: could not locate engine.wasm from OUT_DIR ({}).\n\
                     Build the engine first:\n  \
                     cargo build --manifest-path features/shell/engine/Cargo.toml \
                     --target wasm32-unknown-unknown --release\n\
                     Or set ENGINE_WASM to point to the compiled WASM file.",
                    out_dir.display()
                );
            }
        }
    };

    if !wasm_src.exists() {
        panic!(
            "build.rs: engine.wasm not found at {}\n\
             Build the engine first:\n  \
             cargo build --manifest-path features/shell/engine/Cargo.toml \
             --target wasm32-unknown-unknown --release\n\
             Or set ENGINE_WASM to point to the compiled WASM file.",
            wasm_src.display()
        );
    }

    let dest = out_dir.join("engine.wasm");
    fs::copy(&wasm_src, &dest).expect("build.rs: failed to copy engine.wasm into OUT_DIR");

    println!("cargo:rerun-if-changed={}", wasm_src.display());
    println!("cargo:rerun-if-env-changed=ENGINE_WASM");
}
