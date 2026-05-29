use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt as _};

use crate::detector::{DetectionResult, detect};
use crate::framing::{read_frame, write_frame};
use crate::synthesiser::{SynthesisResult, synthesise};

struct RouterState {
    workspace_root: Option<PathBuf>,
    /// True once Bun context has been detected.
    bun_active: bool,
    /// Held between `initialize` (lockfile detected) and `initialized` (config sent).
    pending_synthesis: Option<SynthesisResult>,
    detection_cache: HashMap<String, DetectionResult>,
}

impl RouterState {
    fn new() -> Self {
        Self {
            workspace_root: None,
            bun_active: false,
            pending_synthesis: None,
            detection_cache: HashMap::new(),
        }
    }
}

pub async fn run(
    ide_in: impl AsyncBufRead + Unpin,
    ide_out: impl AsyncWrite + Unpin,
    lsp_in: impl AsyncWrite + Unpin,
    lsp_out: impl AsyncBufRead + Unpin,
) -> Result<()> {
    tokio::try_join!(
        ide_to_lsp(ide_in, lsp_in, RouterState::new()),
        forward(lsp_out, ide_out, "LSP→IDE"),
    )
    .map(|_| ())
}

async fn ide_to_lsp(
    mut reader: impl AsyncBufRead + Unpin,
    mut lsp_in: impl AsyncWrite + Unpin,
    mut state: RouterState,
) -> Result<()> {
    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(Some(f)) => f,
            Ok(None) => return Ok(()),
            Err(e) => return Err(e),
        };

        let v: Value = match serde_json::from_slice(&frame) {
            Ok(v) => v,
            Err(_) => {
                eprintln!("bundt: IDE→LSP: malformed JSON frame, skipping");
                continue;
            }
        };

        match v.get("method").and_then(Value::as_str) {
            Some("initialize") => on_initialize(&v, &mut lsp_in, &mut state, &frame).await?,
            Some("initialized") => on_initialized(&mut lsp_in, &mut state, &frame).await?,
            Some("textDocument/didOpen") => on_did_open(&v, &mut lsp_in, &mut state, &frame).await?,
            Some("textDocument/didClose") => on_did_close(&v, &mut lsp_in, &mut state, &frame).await?,
            _ => {
                write_frame(&mut lsp_in, &frame).await?;
                lsp_in.flush().await?;
            }
        }
    }
}

async fn on_initialize(
    v: &Value,
    lsp_in: &mut (impl AsyncWrite + Unpin),
    state: &mut RouterState,
    frame: &[u8],
) -> Result<()> {
    let root_uri = v["params"]["rootUri"]
        .as_str()
        .or_else(|| v["params"]["rootPath"].as_str());

    state.workspace_root = root_uri.and_then(uri_to_path);

    if let Some(root) = &state.workspace_root
        && let DetectionResult::Active { signal } = detect("", Some(root.as_path()))
    {
        match synthesise(&signal) {
            Ok(synthesis) => {
                state.bun_active = true;
                state.pending_synthesis = Some(synthesis);
            }
            Err(e) => eprintln!("bundt: synthesise failed at initialize: {e}"),
        }
    }

    write_frame(lsp_in, frame).await?;
    lsp_in.flush().await?;
    Ok(())
}

async fn on_initialized(
    lsp_in: &mut (impl AsyncWrite + Unpin),
    state: &mut RouterState,
    frame: &[u8],
) -> Result<()> {
    if state.bun_active
        && let Some(synthesis) = state.pending_synthesis.take()
    {
        send_configuration(lsp_in, &synthesis).await?;
    }
    write_frame(lsp_in, frame).await?;
    lsp_in.flush().await?;
    Ok(())
}

async fn on_did_open(
    v: &Value,
    lsp_in: &mut (impl AsyncWrite + Unpin),
    state: &mut RouterState,
    frame: &[u8],
) -> Result<()> {
    let uri = v["params"]["textDocument"]["uri"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let text = v["params"]["textDocument"]["text"].as_str().unwrap_or("");

    let result = detect(text, state.workspace_root.as_deref());

    if let DetectionResult::Active { ref signal } = result
        && !state.bun_active
    {
        match synthesise(signal) {
            Ok(synthesis) => {
                send_configuration(lsp_in, &synthesis).await?;
                state.bun_active = true;
            }
            Err(e) => eprintln!("bundt: synthesise failed at textDocument/didOpen: {e}"),
        }
    }

    if !uri.is_empty() {
        state.detection_cache.insert(uri, result);
    }

    write_frame(lsp_in, frame).await?;
    lsp_in.flush().await?;
    Ok(())
}

async fn on_did_close(
    v: &Value,
    lsp_in: &mut (impl AsyncWrite + Unpin),
    state: &mut RouterState,
    frame: &[u8],
) -> Result<()> {
    let uri = v["params"]["textDocument"]["uri"].as_str().unwrap_or("");
    state.detection_cache.remove(uri);
    write_frame(lsp_in, frame).await?;
    lsp_in.flush().await?;
    Ok(())
}

async fn send_configuration(
    lsp_in: &mut (impl AsyncWrite + Unpin),
    synthesis: &SynthesisResult,
) -> Result<()> {
    let notification = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeConfiguration",
        "params": {
            "settings": {
                "typescript": {
                    "tsserver": {
                        "implicitProjectConfig": {
                            "compilerOptions": synthesis.tsconfig
                        }
                    }
                }
            }
        }
    }))?;
    write_frame(lsp_in, &notification).await?;
    lsp_in.flush().await?;
    Ok(())
}

/// Converts a `file://` URI to a filesystem path.
/// Handles `file:///path` and `file://host/path` forms.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        let slash = rest.find('/')?;
        &rest[slash..]
    };
    Some(PathBuf::from(path))
}

async fn forward(
    mut reader: impl AsyncBufRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    label: &'static str,
) -> Result<()> {
    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(Some(f)) => f,
            Ok(None) => return Ok(()),
            Err(e) => return Err(e),
        };
        if serde_json::from_slice::<serde_json::Value>(&frame).is_err() {
            eprintln!("bundt: {label}: malformed JSON frame, skipping");
            continue;
        }
        write_frame(&mut writer, &frame).await?;
        writer.flush().await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    use crate::framing::write_frame;

    // ── helpers ──────────────────────────────────────────────────────────────

    async fn encode(frames: &[&[u8]]) -> Vec<u8> {
        let mut buf = Vec::new();
        for f in frames {
            write_frame(&mut buf, f).await.unwrap();
        }
        buf
    }

    async fn decode_all(bytes: &[u8]) -> Vec<Value> {
        let mut reader = BufReader::new(bytes);
        let mut out = Vec::new();
        while let Some(f) = read_frame(&mut reader).await.unwrap() {
            out.push(serde_json::from_slice(&f).unwrap());
        }
        out
    }

    fn tmp_workspace_with_lockfile() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("bundt-router-test-{}-{}", std::process::id(), id));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bun.lockb"), b"").unwrap();
        dir
    }

    fn file_uri(path: &std::path::Path) -> String {
        format!("file://{}", path.display())
    }

    // ── uri_to_path ───────────────────────────────────────────────────────────

    #[test]
    fn uri_to_path_triple_slash() {
        let p = uri_to_path("file:///home/user/project").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn uri_to_path_with_host() {
        let p = uri_to_path("file://localhost/home/user/project").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn uri_to_path_non_file_scheme_returns_none() {
        assert!(uri_to_path("https://example.com").is_none());
    }

    // ── forwarding ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn unknown_method_forwarded_unchanged() {
        let body = b"{\"method\":\"textDocument/didChange\",\"params\":{}}";
        let input = encode(&[body]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["method"], "textDocument/didChange");
    }

    #[tokio::test]
    async fn lsp_to_ide_forwarded_unchanged() {
        let body = b"{\"id\":1,\"result\":{}}";
        let mut encoded = Vec::new();
        write_frame(&mut encoded, body).await.unwrap();
        let mut reader = BufReader::new(encoded.as_slice());
        let mut ide_out = Vec::new();

        forward(&mut reader, &mut ide_out, "LSP→IDE").await.unwrap();

        let received = read_frame(&mut BufReader::new(ide_out.as_slice()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, body);
    }

    #[tokio::test]
    async fn malformed_json_frame_is_skipped() {
        let bad = b"Content-Length: 3\r\n\r\nnot";
        let good = b"{\"ok\":true}";
        let mut input = bad.to_vec();
        write_frame(&mut input, good).await.unwrap();

        let mut reader = BufReader::new(input.as_slice());
        let mut writer = Vec::new();

        forward(&mut reader, &mut writer, "IDE→LSP").await.unwrap();

        let mut out = BufReader::new(writer.as_slice());
        assert_eq!(read_frame(&mut out).await.unwrap().unwrap(), good);
        assert!(read_frame(&mut out).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn eof_on_ide_input_terminates_run() {
        run(
            BufReader::new(&[][..]),
            Vec::new(),
            Vec::new(),
            BufReader::new(&[][..]),
        )
        .await
        .unwrap();
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn initialize_without_rooturi_does_not_crash() {
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
        let input = encode(&[init]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["method"], "initialize");
    }

    #[tokio::test]
    async fn initialize_without_lockfile_no_config_injected() {
        let dir = std::env::temp_dir().join(format!(
            "bundt-router-no-lock-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let init = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"{}","capabilities":{{}}}}}}"#,
            file_uri(&dir)
        );
        let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let input = encode(&[init.as_bytes(), initialized]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let _ = std::fs::remove_dir_all(&dir);

        let frames = decode_all(&lsp_in).await;
        // Only initialize and initialized — no config notification.
        assert!(
            frames.iter().all(|f| f["method"] != "workspace/didChangeConfiguration"),
            "unexpected config notification: {frames:?}"
        );
    }

    #[tokio::test]
    async fn initialize_with_lockfile_injects_config_at_initialized() {
        let dir = tmp_workspace_with_lockfile();

        let init = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"{}","capabilities":{{}}}}}}"#,
            file_uri(&dir)
        );
        let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let input = encode(&[init.as_bytes(), initialized]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let _ = std::fs::remove_dir_all(&dir);

        let frames = decode_all(&lsp_in).await;
        // Should be: initialize, workspace/didChangeConfiguration, initialized
        assert_eq!(frames.len(), 3, "expected 3 frames, got: {frames:?}");
        assert_eq!(frames[0]["method"], "initialize");
        assert_eq!(frames[1]["method"], "workspace/didChangeConfiguration");
        assert_eq!(frames[2]["method"], "initialized");

        // Config must contain the expected compiler options.
        let compiler_opts = &frames[1]["params"]["settings"]["typescript"]["tsserver"]
            ["implicitProjectConfig"]["compilerOptions"];
        assert_eq!(compiler_opts["moduleResolution"], "bundler");
        assert_eq!(compiler_opts["types"], json!(["bun-types"]));
    }

    // ── textDocument/didOpen ─────────────────────────────────────────────────

    #[tokio::test]
    async fn did_open_shebang_injects_config_before_did_open() {
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
        let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let did_open = br##"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///foo.ts","text":"#!/usr/bin/env bun\n"}}}"##;
        let input = encode(&[init, initialized, did_open]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        // initialize, initialized, workspace/didChangeConfiguration, textDocument/didOpen
        assert_eq!(frames.len(), 4, "expected 4 frames, got: {frames:?}");
        assert_eq!(frames[2]["method"], "workspace/didChangeConfiguration");
        assert_eq!(frames[3]["method"], "textDocument/didOpen");

        let compiler_opts = &frames[2]["params"]["settings"]["typescript"]["tsserver"]
            ["implicitProjectConfig"]["compilerOptions"];
        assert_eq!(compiler_opts["moduleResolution"], "bundler");
    }

    #[tokio::test]
    async fn did_open_non_bun_no_config_injected() {
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
        let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let did_open = br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///foo.ts","text":"const x: number = 1;"}}}"#;
        let input = encode(&[init, initialized, did_open]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        assert!(
            frames.iter().all(|f| f["method"] != "workspace/didChangeConfiguration"),
            "unexpected config notification: {frames:?}"
        );
    }

    #[tokio::test]
    async fn second_bun_did_open_no_duplicate_config() {
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
        let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let open1 = br##"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///a.ts","text":"#!/usr/bin/env bun\n"}}}"##;
        let open2 = br##"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///b.ts","text":"#!/usr/bin/env bun\n"}}}"##;
        let input = encode(&[init, initialized, open1, open2]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        let config_count = frames
            .iter()
            .filter(|f| f["method"] == "workspace/didChangeConfiguration")
            .count();
        assert_eq!(config_count, 1, "expected exactly 1 config notification, got {config_count}");
    }

    // ── textDocument/didClose ─────────────────────────────────────────────────

    #[tokio::test]
    async fn did_close_evicts_uri_and_forwards_frame() {
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
        let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let open = br##"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///a.ts","text":"#!/usr/bin/env bun\n"}}}"##;
        let close = br#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///a.ts"}}}"#;
        let input = encode(&[init, initialized, open, close]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        assert!(
            frames.iter().any(|f| f["method"] == "textDocument/didClose"),
            "didClose not forwarded: {frames:?}"
        );
    }

    #[tokio::test]
    async fn did_close_unknown_uri_does_not_crash() {
        let close = br#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///never-opened.ts"}}}"#;
        let input = encode(&[close]).await;
        let mut lsp_in = Vec::new();

        ide_to_lsp(BufReader::new(input.as_slice()), &mut lsp_in, RouterState::new())
            .await
            .unwrap();

        let frames = decode_all(&lsp_in).await;
        assert_eq!(frames[0]["method"], "textDocument/didClose");
    }
}
