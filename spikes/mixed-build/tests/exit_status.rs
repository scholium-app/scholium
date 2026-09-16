//! CLI regressions: automation must observe failed or empty runs.
use std::process::Command;

#[test]
fn unknown_fixture_is_an_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
        .arg("does-not-exist")
        .output()
        .expect("run spike binary");
    assert!(!output.status.success(), "empty run must not pass");
}

#[test]
fn missing_host_tools_fail_success_fixture() {
    // The compiler now uses a fixed trusted path inside the sandbox. Empty PATH
    // removes host-side PDF probes; failed evidence must still reach the caller.
    let output = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
        .arg("E1-equation")
        .env("PATH", "")
        .output()
        .expect("run spike binary");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Fail"));
    assert!(
        !output.status.success(),
        "failed evidence must reach the caller"
    );
}
