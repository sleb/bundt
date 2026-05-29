use std::process::Command;

fn bundt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bundt"))
}

#[test]
fn help_mentions_bun_default() {
    let out = bundt().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("bun x typescript-language-server"),
        "unexpected help output: {stdout}"
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
