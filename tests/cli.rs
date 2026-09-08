use std::process::Command;

#[test]
fn cli_prints_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_kata-lifecycle"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("ata-lifecycle smoke"));
}
