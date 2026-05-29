use std::process::Stdio;

use bundt::framing::{read_frame, write_frame};
use tokio::io::{AsyncWriteExt, BufReader};

fn find_ts_lsp() -> Option<String> {
    if let Ok(p) = std::env::var("BUNDT_TEST_LSP") {
        return Some(p);
    }
    which::which("typescript-language-server")
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Sends an `initialize` / `shutdown` / `exit` sequence through
/// `bundt <typescript-language-server> --stdio` and asserts that the
/// `initialize` response is a valid JSON-RPC result, and that bundt exits 0.
///
/// Skips if `typescript-language-server` is not on PATH (or `BUNDT_TEST_LSP`).
#[tokio::test]
async fn initialize_round_trip() {
    let Some(ts_lsp) = find_ts_lsp() else {
        eprintln!("initialize_round_trip: skipped (typescript-language-server not found; set BUNDT_TEST_LSP to override)");
        return;
    };

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_bundt"))
        .args([&ts_lsp, "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn bundt");

    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    // initialize
    let init_req =
        br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#;
    write_frame(&mut stdin, init_req).await.unwrap();
    stdin.flush().await.unwrap();

    // Drain server-initiated notifications until the initialize response arrives.
    let value = loop {
        let frame = read_frame(&mut reader)
            .await
            .expect("read_frame failed")
            .expect("no frame received from bundt");
        let v: serde_json::Value =
            serde_json::from_slice(&frame).expect("response is not valid JSON");
        if v.get("id") == Some(&serde_json::json!(1)) {
            break v;
        }
    };

    assert_eq!(value["jsonrpc"], "2.0", "unexpected jsonrpc field: {value}");
    assert!(
        value.get("result").is_some(),
        "response has no 'result' field: {value}"
    );
    assert!(
        value["result"].get("capabilities").is_some(),
        "initialize result missing 'capabilities': {value}"
    );

    // initialized (required by LSP spec before shutdown)
    write_frame(
        &mut stdin,
        br#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
    )
    .await
    .unwrap();

    // shutdown
    write_frame(
        &mut stdin,
        br#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#,
    )
    .await
    .unwrap();
    stdin.flush().await.unwrap();

    // Drain shutdown response (and any server-initiated notifications before it).
    loop {
        let frame = read_frame(&mut reader)
            .await
            .expect("read_frame failed during shutdown")
            .expect("EOF before shutdown response");
        let v: serde_json::Value = serde_json::from_slice(&frame).unwrap_or_default();
        if v.get("id") == Some(&serde_json::json!(2)) {
            break;
        }
    }

    // exit — server must exit with status 0 after a clean shutdown
    write_frame(
        &mut stdin,
        br#"{"jsonrpc":"2.0","method":"exit"}"#,
    )
    .await
    .unwrap();
    stdin.flush().await.unwrap();
    drop(stdin);

    let status = child.wait().await.expect("failed to wait for bundt");
    assert!(status.success(), "bundt exited non-zero: {status}");
}
