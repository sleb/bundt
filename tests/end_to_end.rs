use std::io::Write;
use std::process::Stdio;

use bundt::framing::{read_frame, write_frame};
use tokio::io::BufReader;

fn find_ts_lsp() -> Option<String> {
    if let Ok(p) = std::env::var("BUNDT_TEST_LSP") {
        return Some(p);
    }
    which::which("typescript-language-server")
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

fn initialize_request() -> Vec<u8> {
    br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#.to_vec()
}

/// Sends a single `initialize` request through `bundt <typescript-language-server> --stdio` and
/// asserts that the response is a valid JSON-RPC result for id 1.
///
/// Skips if `typescript-language-server` is not available on PATH (or via BUNDT_TEST_LSP).
#[tokio::test]
async fn initialize_round_trip() {
    let Some(ts_lsp) = find_ts_lsp() else {
        eprintln!("initialize_round_trip: skipped (typescript-language-server not found; set BUNDT_TEST_LSP to override)");
        return;
    };

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_bundt"))
        .args([&ts_lsp, "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn bundt");

    let mut stdin = child.stdin.take().unwrap();

    let mut frame_buf = Vec::new();
    write_frame(&mut frame_buf, &initialize_request()).await.unwrap();
    let exit_notification = br#"{"jsonrpc":"2.0","method":"exit"}"#;
    write_frame(&mut frame_buf, exit_notification).await.unwrap();

    stdin.write_all(&frame_buf).unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "bundt exited non-zero: {}",
        output.status
    );

    let mut reader = BufReader::new(output.stdout.as_slice());
    let frame = read_frame(&mut reader)
        .await
        .expect("read_frame failed")
        .expect("no frame received from bundt");

    let value: serde_json::Value =
        serde_json::from_slice(&frame).expect("response is not valid JSON");

    assert_eq!(value["jsonrpc"], "2.0", "unexpected jsonrpc field: {value}");
    assert_eq!(value["id"], 1, "response id mismatch: {value}");
    assert!(
        value.get("result").is_some(),
        "response has no 'result' field (got error?): {value}"
    );
    assert!(
        value["result"].get("capabilities").is_some(),
        "initialize result missing 'capabilities': {value}"
    );
}
