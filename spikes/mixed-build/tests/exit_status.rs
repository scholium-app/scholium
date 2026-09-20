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
fn sandbox_tools_ignore_host_path_and_setup_failures_reach_the_caller() {
    // Both compilers and PDF probes now use fixed trusted paths in the sandbox.
    let output = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
        .arg("E1-equation")
        .env("PATH", "")
        .output()
        .expect("run spike binary");
    assert!(
        output.status.success(),
        "empty host PATH must not disable sandbox tools:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // A regular file cannot contain sandbox scratch directories. Exercise a
    // real setup failure without altering installed tools or sandbox policy.
    let output = Command::new(env!("CARGO_BIN_EXE_scholium-spike-mixed-build"))
        .arg("E1-equation")
        .env("TMPDIR", "/dev/null")
        .output()
        .expect("run spike binary with unusable scratch directory");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Fail"));
    assert!(
        !output.status.success(),
        "failed evidence must reach the caller"
    );
}
