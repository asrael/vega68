use std::path::Path;

fn die(e: String) -> ! {
    eprintln!("{e}");
    std::process::exit(1);
}

fn report(verb: &str, path: &Path) {
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let root = xtask::repo_root().unwrap_or_default();
    let shown = path.strip_prefix(&root).unwrap_or(path);

    xtask::status(verb, &format!("{} ({bytes} bytes)", shown.display()));
}

fn usage() -> ! {
    eprintln!("usage: cargo xtask <bios | cart <name>>");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| usage());

    match cmd.as_str() {
        "bios" => match xtask::burn_rom() {
            Ok(rom) => report("Burned", &rom),
            Err(e) => die(e),
        },

        "cart" => {
            let name = args.next().unwrap_or_else(|| usage());
            let root = xtask::repo_root().unwrap_or_else(|e| die(e));
            let out_dir = root.join("target/carts");
            let cart_dir = root.join("carts").join(&name);

            std::fs::create_dir_all(&out_dir).expect("failed to create target/carts");

            match xtask::build_cart(&cart_dir, &out_dir) {
                Ok(v68) => report("Built", &v68),
                Err(e) => die(e),
            }
        }

        _ => usage(),
    }
}
