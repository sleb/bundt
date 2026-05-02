use std::process::Command;

fn bundt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bundt"))
}

#[test]
fn no_args_exits_nonzero_with_usage() {
    let out = bundt().output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("binary") || stderr.contains("Usage"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn missing_binary_exits_1_with_not_found() {
    let out = bundt()
        .arg("/bundt-test-does-not-exist")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not found"), "unexpected stderr: {stderr}");
}
