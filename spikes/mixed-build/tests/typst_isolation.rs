//! End-to-end checks through the public mixed snapshot rebuild entry.
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "typst-isolation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("fresh fixture");
        let snapshot = json!({
            "schema": "scholium-spike-snapshot-v1", "max_rounds": 2,
            "project": {"name":"isolation", "host":"Typst", "body":[{"Raw":{"dialect":"Typst","text":source}}],
                "macros":[], "components":[], "page":[595.0,842.0], "unscoped_control":false}
        });
        fs::write(
            path.join("snapshot.json"),
            serde_json::to_vec(&snapshot).expect("serialize"),
        )
        .expect("request");
        Self(path)
    }
    fn run(&self) -> std::process::Output {
        // An outer harness deadline catches regressions without hanging cargo test.
        Command::new("/usr/bin/timeout")
            .args([
                "--kill-after=2s",
                "20s",
                env!("CARGO_BIN_EXE_scholium-spike-mixed-build"),
                "--rebuild",
            ])
            .arg(&self.0)
            .arg(self.0.join("rebuilt"))
            .output()
            .expect("driver")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn ordinary_typst_rebuild_still_publishes_a_pdf() {
    let fixture = Fixture::new("BENIGNCONTROL");
    let result = fixture.run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        fs::read(fixture.0.join("rebuilt/final.pdf"))
            .expect("pdf")
            .starts_with(b"%PDF-")
    );
}

#[test]
fn expensive_typst_is_stopped_by_worker_budget_without_publishing() {
    // A non-allocating loop isolates wall time from the independent memory limit.
    let fixture = Fixture::new(
        "#{ for a in range(10000) { for b in range(10000) { for c in range(10000) { let x = a + b + c } } } }",
    );
    let result = fixture.run();
    assert!(!result.status.success());
    assert_ne!(
        result.status.code(),
        Some(124),
        "outer harness had to stop the driver"
    );
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("typst-worker-timeout"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!fixture.0.join("rebuilt/final.pdf").exists());
}

#[test]
fn undeclared_resources_do_not_become_available_inside_worker() {
    let fixture = Fixture::new("#read(\"/etc/hostname\")");
    let result = fixture.run();
    assert!(!result.status.success());
    assert_ne!(result.status.code(), Some(124));
    assert!(String::from_utf8_lossy(&result.stderr).contains("file not found"));
    assert!(!fixture.0.join("rebuilt/final.pdf").exists());
}
