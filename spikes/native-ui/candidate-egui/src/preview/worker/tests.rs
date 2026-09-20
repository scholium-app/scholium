use super::*;

#[test]
fn png_reader_rejects_oversize_dimensions_symlinks_and_wrong_magic() {
    let root = std::env::temp_dir().join(format!("scholium-png-test-{}", std::process::id()));
    fs::create_dir(&root).expect("directory");
    let path = root.join("page-1.png");
    image::RgbaImage::new(2, 2).save(&path).expect("valid PNG");
    assert_eq!(read_pages(&root).expect("valid page")[0].size, [2, 2]);
    image::RgbaImage::new(4097, 1)
        .save(&path)
        .expect("wide PNG");
    assert!(read_pages(&root).is_err());
    fs::write(&path, b"not a PNG").expect("bad magic");
    assert!(read_pages(&root).is_err());
    fs::rename(&path, root.join("payload")).expect("move payload");
    std::os::unix::fs::symlink(root.join("payload"), &path).expect("symlink");
    assert!(read_pages(&root).is_err());
    fs::remove_file(&path).expect("remove link");
    image::RgbaImage::new(2, 2)
        .save(root.join("page-2.png"))
        .expect("gap PNG");
    assert!(read_pages(&root).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}
