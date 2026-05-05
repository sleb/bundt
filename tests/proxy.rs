use std::io::Write;
use std::process::{Command, Stdio};

use bundt::framing::{read_frame, write_frame};
use tokio::io::BufReader;

fn bundt_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bundt"))
}

fn lsp_echo_path() -> &'static str {
    env!("CARGO_BIN_EXE_lsp_echo")
}

#[tokio::test]
async fn three_frames_forwarded_unchanged() {
    let bodies: &[&[u8]] = &[b"{\"id\":1}", b"{\"id\":2}", b"{\"id\":3}"];

    let mut child = bundt_cmd()
        .arg(lsp_echo_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    for body in bodies {
        let mut frame = Vec::new();
        write_frame(&mut frame, body).await.unwrap();
        stdin.write_all(&frame).unwrap();
    }
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "bundt exited: {}", output.status);

    let mut reader = BufReader::new(output.stdout.as_slice());
    for expected in bodies {
        let frame = read_frame(&mut reader).await.unwrap().unwrap();
        assert_eq!(&frame, expected);
    }
    assert!(read_frame(&mut reader).await.unwrap().is_none());
}

#[tokio::test]
async fn malformed_json_frame_skipped_valid_frames_still_arrive() {
    let bad_frame = b"Content-Length: 3\r\n\r\nnot";
    let good_before = b"{\"id\":\"before\"}";
    let good_after = b"{\"id\":\"after\"}";

    let mut child = bundt_cmd()
        .arg(lsp_echo_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut buf = Vec::new();
    write_frame(&mut buf, good_before).await.unwrap();
    stdin.write_all(&buf).unwrap();
    stdin.write_all(bad_frame).unwrap();
    buf.clear();
    write_frame(&mut buf, good_after).await.unwrap();
    stdin.write_all(&buf).unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
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
    let mut child = bundt_cmd()
        .arg(lsp_echo_path())
        .args(["--exit-after", "1", "--exit-code", "42"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut frame = Vec::new();
    write_frame(&mut frame, b"{\"id\":1}").await.unwrap();
    stdin.write_all(&frame).unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(42), "expected exit 42, got: {}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("42"), "expected exit code in stderr, got: {stderr}");
}

#[tokio::test]
async fn clean_shutdown_exits_0() {
    let mut child = bundt_cmd()
        .arg(lsp_echo_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "expected exit 0, got: {}", output.status);
}
