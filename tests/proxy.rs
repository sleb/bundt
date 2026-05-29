use std::io::Write as _;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};

use bundt::framing::{read_frame, write_frame};
use serde_json::Value;
use tokio::io::BufReader;

fn bundt_path() -> &'static str {
    env!("CARGO_BIN_EXE_bundt")
}

fn lsp_echo_path() -> &'static str {
    env!("CARGO_BIN_EXE_lsp_echo")
}

/// Spawn `bundt <args>` synchronously, write `stdin_bytes` to stdin, close it,
/// and collect the full output. Suitable for `tokio::task::spawn_blocking`.
///
/// Using `std::process::Command` (not Tokio's) keeps each test's child process
/// lifecycle entirely separate: no shared Tokio SIGCHLD handling, no cross-test
/// reactor interactions.
fn run_bundt(args: Vec<String>, stdin_bytes: Vec<u8>) -> std::process::Output {
    let mut child = std::process::Command::new(bundt_path())
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn bundt");

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&stdin_bytes).unwrap();
    drop(stdin);

    child.wait_with_output().unwrap()
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn encode_frames(frames: &[&[u8]]) -> Vec<u8> {
    let mut buf = Vec::new();
    for f in frames {
        write_frame(&mut buf, f).await.unwrap();
    }
    buf
}

async fn decode_all_frames(bytes: &[u8]) -> Vec<Value> {
    let mut reader = BufReader::new(bytes);
    let mut out = Vec::new();
    while let Some(f) = read_frame(&mut reader).await.unwrap() {
        if let Ok(v) = serde_json::from_slice::<Value>(&f) {
            out.push(v);
        }
    }
    out
}

fn tmp_dir_with_lockfile() -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join(format!("bundt-proxy-test-{}-{}", std::process::id(), id));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bun.lockb"), b"").unwrap();
    dir
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

// ── forwarding tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn three_frames_forwarded_unchanged() {
    let bodies: &[&[u8]] = &[b"{\"id\":1}", b"{\"id\":2}", b"{\"id\":3}"];

    let mut stdin_bytes = Vec::new();
    for body in bodies {
        write_frame(&mut stdin_bytes, body).await.unwrap();
    }

    let output = tokio::task::spawn_blocking(move || {
        run_bundt(vec![lsp_echo_path().to_owned()], stdin_bytes)
    })
    .await
    .unwrap();

    assert!(output.status.success(), "bundt exited: {}", output.status);

    let stdout_len = output.stdout.len();
    let stderr_text = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut reader = BufReader::new(output.stdout.as_slice());
    for (i, expected) in bodies.iter().enumerate() {
        let frame = read_frame(&mut reader).await.unwrap().unwrap_or_else(|| {
            panic!("frame {i} missing (stdout={stdout_len}B stderr={stderr_text:?})")
        });
        assert_eq!(&frame, expected);
    }
    assert!(read_frame(&mut reader).await.unwrap().is_none());
}

#[tokio::test]
async fn malformed_json_frame_skipped_valid_frames_still_arrive() {
    let bad_frame = b"Content-Length: 3\r\n\r\nnot";
    let good_before = b"{\"id\":\"before\"}";
    let good_after = b"{\"id\":\"after\"}";

    let mut stdin_bytes = Vec::new();
    write_frame(&mut stdin_bytes, good_before).await.unwrap();
    stdin_bytes.extend_from_slice(bad_frame);
    write_frame(&mut stdin_bytes, good_after).await.unwrap();

    let output = tokio::task::spawn_blocking(move || {
        run_bundt(vec![lsp_echo_path().to_owned()], stdin_bytes)
    })
    .await
    .unwrap();

    assert!(output.status.success(), "bundt exited: {}", output.status);

    let mut reader = BufReader::new(output.stdout.as_slice());
    let f1 = read_frame(&mut reader).await.unwrap().unwrap();
    assert_eq!(f1, good_before);
    let f2 = read_frame(&mut reader).await.unwrap().unwrap();
    assert_eq!(f2, good_after);
    assert!(read_frame(&mut reader).await.unwrap().is_none());
}

#[tokio::test]
async fn lsp_exit_code_propagated_to_bundt() {
    let mut stdin_bytes = Vec::new();
    write_frame(&mut stdin_bytes, b"{\"id\":1}").await.unwrap();

    let output = tokio::task::spawn_blocking(move || {
        run_bundt(
            vec![
                lsp_echo_path().to_owned(),
                "--exit-after".to_owned(),
                "1".to_owned(),
                "--exit-code".to_owned(),
                "42".to_owned(),
            ],
            stdin_bytes,
        )
    })
    .await
    .unwrap();

    assert_eq!(
        output.status.code(),
        Some(42),
        "expected exit 42, got: {}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("42"),
        "expected exit code in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn clean_shutdown_exits_0() {
    let output = tokio::task::spawn_blocking(|| {
        run_bundt(vec![lsp_echo_path().to_owned()], Vec::new())
    })
    .await
    .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0, got: {}",
        output.status
    );
}

// ── Bun injection tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn lockfile_workspace_injects_configuration_after_initialized() {
    let dir = tmp_dir_with_lockfile();
    let init = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"{}","capabilities":{{}}}}}}"#,
        file_uri(&dir)
    );
    let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let stdin_bytes = encode_frames(&[init.as_bytes(), initialized]).await;

    let output = tokio::task::spawn_blocking(move || {
        run_bundt(vec![lsp_echo_path().to_owned()], stdin_bytes)
    })
    .await
    .unwrap();

    let _ = std::fs::remove_dir_all(&dir);
    assert!(output.status.success(), "bundt exited: {}", output.status);

    let frames = decode_all_frames(&output.stdout).await;
    assert!(
        frames.iter().any(|f| f["method"] == "workspace/didChangeConfiguration"),
        "expected config notification in output frames: {frames:?}"
    );

    // Config must arrive before the echoed initialized frame.
    let config_pos = frames
        .iter()
        .position(|f| f["method"] == "workspace/didChangeConfiguration")
        .unwrap();
    let initialized_pos = frames
        .iter()
        .position(|f| f["method"] == "initialized")
        .unwrap();
    assert!(
        config_pos < initialized_pos,
        "config ({config_pos}) should precede initialized ({initialized_pos})"
    );
}

#[tokio::test]
async fn shebang_did_open_injects_configuration() {
    let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
    let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = br##"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///script.ts","languageId":"typescript","version":1,"text":"#!/usr/bin/env bun\n"}}}"##;
    let stdin_bytes = encode_frames(&[init, initialized, did_open]).await;

    let output = tokio::task::spawn_blocking(move || {
        run_bundt(vec![lsp_echo_path().to_owned()], stdin_bytes)
    })
    .await
    .unwrap();

    assert!(output.status.success(), "bundt exited: {}", output.status);

    let frames = decode_all_frames(&output.stdout).await;
    assert!(
        frames.iter().any(|f| f["method"] == "workspace/didChangeConfiguration"),
        "expected config notification for shebang file: {frames:?}"
    );

    // Config must arrive before the echoed didOpen frame.
    let config_pos = frames
        .iter()
        .position(|f| f["method"] == "workspace/didChangeConfiguration")
        .unwrap();
    let did_open_pos = frames
        .iter()
        .position(|f| f["method"] == "textDocument/didOpen")
        .unwrap();
    assert!(
        config_pos < did_open_pos,
        "config ({config_pos}) should precede didOpen ({did_open_pos})"
    );
}

#[tokio::test]
async fn non_bun_did_open_no_configuration_injected() {
    let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootUri":null,"capabilities":{}}}"#;
    let initialized = br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///app.ts","languageId":"typescript","version":1,"text":"const x: number = 1;"}}}"#;
    let stdin_bytes = encode_frames(&[init, initialized, did_open]).await;

    let output = tokio::task::spawn_blocking(move || {
        run_bundt(vec![lsp_echo_path().to_owned()], stdin_bytes)
    })
    .await
    .unwrap();

    assert!(output.status.success(), "bundt exited: {}", output.status);

    let frames = decode_all_frames(&output.stdout).await;
    assert!(
        frames
            .iter()
            .all(|f| f["method"] != "workspace/didChangeConfiguration"),
        "unexpected config notification for non-Bun file: {frames:?}"
    );
}
