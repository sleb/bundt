use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::detector::Signal;

include!(concat!(env!("OUT_DIR"), "/bun_types_generated.rs"));

pub struct SynthesisResult {
    pub tsconfig: Value,
    pub types_root: PathBuf,
}

/// Returns the directory that serves as the `typeRoots` entry.
///
/// Layout on disk:
///   {data_dir}/bundt/bun-types-{VERSION}/
///     bun-types/
///       index.d.ts
///       …
///
/// The directory is created and populated on first call; subsequent calls
/// skip extraction if the versioned directory already exists.
pub fn types_root() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("could not determine user data directory")?;
    let root = data_dir
        .join("bundt")
        .join(format!("bun-types-{}", BUN_TYPES_VERSION));

    if root.exists() {
        return Ok(root);
    }

    let pkg_dir = root.join("bun-types");
    fs::create_dir_all(&pkg_dir).with_context(|| {
        format!("failed to create bun-types extraction directory: {}", pkg_dir.display())
    })?;

    for (rel_path, bytes) in BUN_TYPES_FILES {
        let dest = pkg_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, bytes)
            .with_context(|| format!("failed to write {}", dest.display()))?;
    }

    Ok(root)
}

pub fn synthesise(_signal: &Signal) -> Result<SynthesisResult> {
    let root = types_root()?;
    let types_root_str = root.to_string_lossy();

    let tsconfig = json!({
        "moduleResolution": "bundler",
        "target": "ESNext",
        "lib": ["ESNext"],
        "resolveJsonModule": true,
        "typeRoots": [types_root_str],
        "types": ["bun-types"]
    });

    Ok(SynthesisResult {
        tsconfig,
        types_root: root,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector::Signal;

    fn run_synthesise(signal: Signal) -> SynthesisResult {
        synthesise(&signal).expect("synthesise failed")
    }

    #[test]
    fn required_fields_present() {
        let result = run_synthesise(Signal::Shebang);
        let cfg = &result.tsconfig;
        assert_eq!(cfg["moduleResolution"], "bundler");
        assert_eq!(cfg["target"], "ESNext");
        assert_eq!(cfg["lib"], json!(["ESNext"]));
        assert_eq!(cfg["resolveJsonModule"], true);
        assert_eq!(cfg["types"], json!(["bun-types"]));
    }

    #[test]
    fn type_roots_contains_extraction_path() {
        let result = run_synthesise(Signal::Shebang);
        let roots = result.tsconfig["typeRoots"].as_array().unwrap();
        assert_eq!(roots.len(), 1);
        let root_str = roots[0].as_str().unwrap();
        assert!(
            root_str.ends_with(&format!("bun-types-{}", BUN_TYPES_VERSION)),
            "unexpected typeRoots path: {root_str}"
        );
    }

    #[test]
    fn types_root_matches_tsconfig() {
        let result = run_synthesise(Signal::Shebang);
        let from_tsconfig = result.tsconfig["typeRoots"][0].as_str().unwrap();
        assert_eq!(result.types_root.to_string_lossy().as_ref(), from_tsconfig);
    }

    #[test]
    fn all_signals_produce_same_tsconfig() {
        let shebang = run_synthesise(Signal::Shebang).tsconfig;
        let bun_import = run_synthesise(Signal::BunImport).tsconfig;
        let lockfile = run_synthesise(Signal::Lockfile).tsconfig;
        assert_eq!(shebang, bun_import);
        assert_eq!(shebang, lockfile);
    }

    #[test]
    fn extraction_directory_contains_index_dts() {
        let result = run_synthesise(Signal::Shebang);
        let index = result.types_root.join("bun-types").join("index.d.ts");
        assert!(index.exists(), "index.d.ts not found at {}", index.display());
    }

    #[test]
    fn extraction_is_idempotent() {
        // Calling synthesise twice must not error (directory already exists).
        run_synthesise(Signal::BunImport);
        run_synthesise(Signal::Lockfile);
    }
}
