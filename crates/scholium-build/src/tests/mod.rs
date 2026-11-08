mod native;

use crate::{BuildError, cache};

#[test]
fn font_cache_copies_are_independent_and_reject_symlinks() {
    let cache = tempfile::tempdir().expect("cache");
    let copy = tempfile::tempdir().expect("copy");
    std::fs::write(cache.path().join("font"), "trusted").expect("cached data");
    cache::copy(cache.path(), copy.path()).expect("copy");
    std::fs::write(copy.path().join("font"), "poisoned").expect("mutate session");
    assert_eq!(
        std::fs::read_to_string(cache.path().join("font")).expect("trusted data"),
        "trusted"
    );
    std::os::unix::fs::symlink("/etc/passwd", cache.path().join("escape")).expect("link");
    let other = tempfile::tempdir().expect("copy");
    assert!(matches!(
        cache::copy(cache.path(), other.path()),
        Err(BuildError::Configuration(_))
    ));
}

mod typst_native;
mod typst_profile;

mod quota;
