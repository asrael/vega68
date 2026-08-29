use std::path::{Path, PathBuf};

fn plant(case: &str, files: &[&str]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(case);

    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    for file in files {
        let path = dir.join(file);

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    dir
}

#[test]
fn a_directory_with_main_c_is_a_cart() {
    let dir = plant("has_main", &["main.c", "util.c"]);
    let sources = xtask::cart_sources(&dir).unwrap();

    let names: Vec<String> = sources
        .iter()
        .map(|p| {
            p.strip_prefix(&dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();

    assert_eq!(names, ["main.c", "util.c"]);
}

#[test]
fn a_directory_without_main_c_is_not_a_cart() {
    let dir = plant("no_main", &["util.c"]);
    let e = xtask::cart_sources(&dir).unwrap_err();

    assert!(e.contains("main.c"), "{e}");
}
