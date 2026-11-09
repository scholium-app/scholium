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

#[test]
fn reported_pages_reads_the_last_engine_summary_even_when_tex_wraps_the_line() {
    use crate::full::reported_pages;
    assert_eq!(
        reported_pages("Output written on /work/preview.pdf (3 pages, 41234 bytes)."),
        Some(3)
    );
    assert_eq!(
        reported_pages("Output written on /work/preview.xdv (1 page, 912 bytes)."),
        Some(1)
    );
    let wrapped =
        "Output written on /work/a-very-long-directory-name/preview.pdf (31\n2 pages, 1 bytes).";
    assert_eq!(reported_pages(wrapped), Some(312));
    let two =
        "Output written on a.pdf (2 pages, 1 bytes).\nOutput written on a.pdf (5 pages, 1 bytes).";
    assert_eq!(reported_pages(two), Some(5));
    assert_eq!(reported_pages("No pages of output."), None);
    assert_eq!(
        reported_pages("Output written on a.pdf (many pages)."),
        None
    );
}
