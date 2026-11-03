use crate::full::snapshot;
#[test]
fn full_snapshot_rejects_symlinks_and_resource_budget_without_modifying_source() {
    let source = tempfile::tempdir().expect("source");
    let target = tempfile::tempdir().expect("target");
    std::fs::write(source.path().join("main.tex"), "source").expect("write");
    std::os::unix::fs::symlink("/etc/passwd", source.path().join("escape")).expect("link");
    assert!(snapshot(source.path(), source.path(), target.path(), &mut (0, 0)).is_err());
    std::fs::remove_file(source.path().join("escape")).expect("unlink");
    assert!(snapshot(source.path(), source.path(), target.path(), &mut (10000, 0)).is_err());
    assert_eq!(
        std::fs::read_to_string(source.path().join("main.tex")).expect("source"),
        "source"
    );
}
