use std::path::Path;

/// The signal that caused Bun context to be detected.
///
/// Evaluated in priority order: Shebang > BunImport > Lockfile.
/// Only the highest-priority matching signal is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// `#!/usr/bin/env bun` on the first line of the file.
    Shebang,
    /// A static ES module import or re-export from the `"bun"` specifier.
    BunImport,
    /// `bun.lockb` exists in the workspace root.
    Lockfile,
}

/// Whether a file is in Bun context and, if so, which signal triggered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionResult {
    Inactive,
    Active { signal: Signal },
}

/// Decide whether `file_content` (and optionally its workspace) is Bun context.
///
/// Signals are evaluated in priority order and the function returns on the
/// first match. The lockfile check is the only filesystem read; shebang and
/// import checks operate solely on `file_content`.
pub fn detect(file_content: &str, workspace_root: Option<&Path>) -> DetectionResult {
    if is_bun_shebang(file_content) {
        return DetectionResult::Active { signal: Signal::Shebang };
    }
    if has_bun_import(file_content) {
        return DetectionResult::Active { signal: Signal::BunImport };
    }
    if has_bun_lockfile(workspace_root) {
        return DetectionResult::Active { signal: Signal::Lockfile };
    }
    DetectionResult::Inactive
}

/// Returns true iff the very first line is exactly `#!/usr/bin/env bun`.
/// Leading whitespace and extra arguments (e.g. `#!/usr/bin/env bun --smol`)
/// are not accepted.
fn is_bun_shebang(content: &str) -> bool {
    content.lines().next().unwrap_or("") == "#!/usr/bin/env bun"
}

/// Returns true if `content` contains a static ES module import or re-export
/// from the `"bun"` specifier.
///
/// Matched forms (single and double quotes):
///   import ... from "bun" / 'bun'
///   import "bun" / 'bun'
///   export ... from "bun" / 'bun'   ← covered by the `from` check
///
/// `require("bun")` is intentionally not matched — CommonJS require is not a
/// Bun-specific signal. Dynamic `import("bun")` is also not matched.
///
/// The check is a fast string scan and may produce false positives for
/// occurrences inside comments or string literals. That is acceptable: a false
/// positive activates Bun support on a file that doesn't need it, which is
/// benign.
fn has_bun_import(content: &str) -> bool {
    content.contains(r#"from "bun""#)
        || content.contains("from 'bun'")
        || content.contains(r#"import "bun""#)
        || content.contains("import 'bun'")
}

/// Returns true if `bun.lockb` exists as a file at `workspace_root`.
/// Returns false (not an error) if `workspace_root` is `None` or the file is
/// absent. Unexpected I/O errors are logged at debug level and treated as absent.
fn has_bun_lockfile(workspace_root: Option<&Path>) -> bool {
    let Some(root) = workspace_root else {
        return false;
    };
    match std::fs::metadata(root.join("bun.lockb")) {
        Ok(meta) => meta.is_file(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            eprintln!("bundt: lockfile check failed: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Create a temp directory containing `bun.lockb` and return its path.
    /// The caller is responsible for cleanup (drop or explicit remove).
    fn tmp_workspace_with_lockfile() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("bundt-detector-test-{}-{}", std::process::id(), id));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("bun.lockb"), b"").unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // ── shebang ──────────────────────────────────────────────────────────────

    #[test]
    fn shebang_first_line_exact_match() {
        let result = detect("#!/usr/bin/env bun\nconst x = 1;", None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::Shebang });
    }

    #[test]
    fn shebang_only_line() {
        let result = detect("#!/usr/bin/env bun", None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::Shebang });
    }

    #[test]
    fn shebang_on_second_line_is_not_detected() {
        // Falls through to BunImport / Lockfile checks; neither fires here.
        let result = detect("\n#!/usr/bin/env bun\n", None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    #[test]
    fn shebang_with_extra_arg_is_not_detected() {
        let result = detect("#!/usr/bin/env bun --smol\nconst x = 1;", None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    #[test]
    fn shebang_with_leading_whitespace_is_not_detected() {
        let result = detect(" #!/usr/bin/env bun\n", None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    // ── bun import ───────────────────────────────────────────────────────────

    #[test]
    fn named_import_double_quote() {
        let result = detect(r#"import { $ } from "bun";"#, None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::BunImport });
    }

    #[test]
    fn named_import_single_quote() {
        let result = detect("import { $ } from 'bun';", None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::BunImport });
    }

    #[test]
    fn bare_import_double_quote() {
        let result = detect(r#"import "bun";"#, None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::BunImport });
    }

    #[test]
    fn bare_import_single_quote() {
        let result = detect("import 'bun';", None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::BunImport });
    }

    #[test]
    fn reexport_from_bun() {
        let result = detect(r#"export { serve } from "bun";"#, None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::BunImport });
    }

    #[test]
    fn import_from_bun_utils_is_not_detected() {
        // "bun-utils" must not match the `from "bun"` check.
        let result = detect(r#"import { foo } from "bun-utils";"#, None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    #[test]
    fn require_bun_is_not_detected() {
        let result = detect(r#"const bun = require("bun");"#, None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    #[test]
    fn dynamic_import_bun_is_not_detected() {
        let result = detect(r#"const b = await import("bun");"#, None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    // ── lockfile ─────────────────────────────────────────────────────────────

    #[test]
    fn lockfile_present_at_workspace_root() {
        let dir = tmp_workspace_with_lockfile();
        let result = detect("const x = 1;", Some(&dir));
        cleanup(&dir);
        assert_eq!(result, DetectionResult::Active { signal: Signal::Lockfile });
    }

    #[test]
    fn lockfile_absent_at_workspace_root() {
        let dir = std::env::temp_dir()
            .join(format!("bundt-detector-test-no-lock-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let result = detect("const x = 1;", Some(&dir));
        cleanup(&dir);
        assert_eq!(result, DetectionResult::Inactive);
    }

    #[test]
    fn lockfile_check_skipped_when_workspace_root_is_none() {
        // Even if a bun.lockb happened to exist somewhere, no workspace → no lockfile check.
        let result = detect("const x = 1;", None);
        assert_eq!(result, DetectionResult::Inactive);
    }

    // ── inactive ─────────────────────────────────────────────────────────────

    #[test]
    fn empty_content_no_workspace_is_inactive() {
        assert_eq!(detect("", None), DetectionResult::Inactive);
    }

    // ── priority ordering ────────────────────────────────────────────────────

    #[test]
    fn shebang_beats_bun_import() {
        let content = "#!/usr/bin/env bun\nimport { $ } from \"bun\";";
        let result = detect(content, None);
        assert_eq!(result, DetectionResult::Active { signal: Signal::Shebang });
    }

    #[test]
    fn bun_import_beats_lockfile() {
        let dir = tmp_workspace_with_lockfile();
        let result = detect(r#"import { $ } from "bun";"#, Some(&dir));
        cleanup(&dir);
        assert_eq!(result, DetectionResult::Active { signal: Signal::BunImport });
    }
}
