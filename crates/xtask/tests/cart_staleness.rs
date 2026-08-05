use std::path::Path;

fn tool_available(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .is_ok()
}

#[test]
fn a_fresh_cart_is_not_recompiled() {
    if !tool_available("m68k-elf-gcc") {
        eprintln!("skipping: m68k-elf-gcc not on PATH");
        return;
    }

    let cart_dir = xtask::repo_root().unwrap().join("carts/hello");
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cart_staleness_hello");

    let v68 = xtask::build_cart(&cart_dir, &out_dir).unwrap();
    let first = std::fs::metadata(&v68).unwrap().modified().unwrap();

    let v68_again = xtask::build_cart(&cart_dir, &out_dir).unwrap();
    let second = std::fs::metadata(&v68_again).unwrap().modified().unwrap();

    assert_eq!(v68, v68_again);
    assert_eq!(
        first, second,
        "second build_cart rewrote an artifact whose inputs had not changed"
    );
}
