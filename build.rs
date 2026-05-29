use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=vendor/bun-types");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("bun_types_generated.rs");

    let vendor_dir = Path::new("vendor/bun-types");

    // Extract version from package.json
    let pkg_json = fs::read_to_string(vendor_dir.join("package.json"))
        .expect("vendor/bun-types/package.json missing");
    let version = extract_version(&pkg_json).expect("version field missing in package.json");

    // Collect all .d.ts files, sorted for determinism
    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect_dts_files(vendor_dir, vendor_dir, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate the Rust source
    let mut out = String::new();
    out.push_str("pub static BUN_TYPES_FILES: &[(&str, &[u8])] = &[\n");
    for (rel_path, abs_path) in &entries {
        out.push_str(&format!(
            "    ({:?}, include_bytes!({:?})),\n",
            rel_path,
            abs_path.to_str().unwrap(),
        ));
    }
    out.push_str("];\n");
    out.push_str(&format!("pub const BUN_TYPES_VERSION: &str = {:?};\n", version));

    fs::write(&out_path, out).expect("failed to write bun_types_generated.rs");
}

fn collect_dts_files(base: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) {
    for entry in fs::read_dir(dir).expect("failed to read vendor dir") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_dts_files(base, &path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("ts") {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let abs = fs::canonicalize(&path).unwrap();
            out.push((rel, abs));
        }
    }
}

/// Minimal version extraction from `{"version": "x.y.z", ...}` without pulling in serde.
fn extract_version(json: &str) -> Option<String> {
    let key = "\"version\"";
    let start = json.find(key)? + key.len();
    let rest = json[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
