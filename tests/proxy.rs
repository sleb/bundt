use std::process::{Command, Stdio};

use bundt::framing::{read_frame, write_frame};
use tokio::io::BufReader;

fn bundt_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bundt"))
}

fn lsp_echo_path() -> &'static str {
    env!("CARGO_BIN_EXE_lsp_echo")
}

async fn encode(body: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_frame(&mut buf, body).await.unwrap();
    buf
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
        let frame = encode(body).await;
        std::io::Write::write_all(&mut stdin, &frame).unwrap();
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
    std::io::Write::write_all(&mut stdin, &encode(good_before).await).unwrap();
    std::io::Write::write_all(&mut stdin, bad_frame).unwrap();
    std::io::Write::write_all(&mut stdin, &encode(good_after).await).unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    // malformed frame is skipped but bundt must not exit non-zero
    assert!(output.status.success(), "bundt exited: {}", output.status);

    let mut reader = BufReader::new(output.stdout.as_slice());
    let f1 = read_frame(&mut reader).await.unwrap().unwrap();
    assert_eq!(f1, good_before);
    let f2 = read_frame(&mut reader).await.unwrap().unwrap();
    assert_eq!(f2, good_after);
    assert!(read_frame(&mut reader).await.unwrap().is_none());
}
