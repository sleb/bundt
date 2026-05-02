use std::process::Stdio;

use bundt::framing::{read_frame, write_frame};
use tokio::io::BufReader;

fn find_vtsls() -> Option<String> {
    // Honour an explicit override first (useful in CI or when vtsls is not on PATH).
    if let Ok(p) = std::env::var("BUNDT_TEST_LSP") {
        return Some(p);
    }
    which::which("vtsls").ok().map(|p| p.to_string_lossy().into_owned())
}

/// Minimal `initialize` request body (no workspace root, no capabilities).
fn initialize_request() -> Vec<u8> {
    br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#.to_vec()
}

/// Sends a single `initialize` request through `bundt <vtsls> --stdio` and
/// asserts that the response is a valid JSON-RPC result for id 1.
///
/// Skips if `vtsls` is not available on PATH (or via BUNDT_TEST_LSP).
#[tokio::test]
async fn initialize_round_trip() {
    let Some(vtsls) = find_vtsls() else {
        eprintln!("initialize_round_trip: skipped (vtsls not found; set BUNDT_TEST_LSP to override)");
        return;
    };

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_bundt"))
        .args([&vtsls, "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn bundt");

    let mut stdin = child.stdin.take().unwrap();

    // Send initialize then immediately close stdin so bundt shuts down after
    // the LSP processes the request and returns a response.
    let mut frame_buf = Vec::new();
    write_frame(&mut frame_buf, &initialize_request()).await.unwrap();

    // Also send an `exit` notification so vtsls shuts down cleanly.
    let exit_notification = br#"{"jsonrpc":"2.0","method":"exit"}"#;
    write_frame(&mut frame_buf, exit_notification).await.unwrap();

    std::io::Write::write_all(&mut stdin, &frame_buf).unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();

    // bundt should exit 0 (vtsls cleans up on `exit`).
    assert!(
        output.status.success(),
        "bundt exited non-zero: {}",
        output.status
    );

    // Parse the first response frame: must be a JSON-RPC result for id 1.
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
