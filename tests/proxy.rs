use std::io::Write as _;
use std::process::Stdio;

use bundt::framing::{read_frame, write_frame};
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
