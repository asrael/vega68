use std::path::Path;
use std::time::SystemTime;

fn die(e: String) -> ! {
    eprintln!("{e}");
    std::process::exit(1);
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn report(verb: &str, path: &Path) {
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let root = xtask::repo_root().unwrap_or_default();
    let shown = path.strip_prefix(&root).unwrap_or(path);

    xtask::status(verb, &format!("{} ({bytes} bytes)", shown.display()));
}

fn usage() -> ! {
    eprintln!(
        "usage: cargo xtask <bios | bundle <name> [--release] [--run <args>] | cart <name> [--watch]>"
    );
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

        "bundle" => {
            let name = args.next().unwrap_or_else(|| usage());
            let root = xtask::repo_root().unwrap_or_else(|e| die(e));
            let mut release = false;
            let mut run = false;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--release" => release = true,
                    "--run" => {
                        run = true;
                        break;
                    }
                    _ => usage(),
                }
            }

            let bin = match xtask::bundle(&root.join("carts").join(name), release) {
                Ok(bin) => bin,
                Err(e) => die(e),
            };

            report("Bundled", &bin);

            if run {
                let status = std::process::Command::new(&bin)
                    .args(args)
                    .status()
                    .unwrap_or_else(|e| die(format!("{}: {e}", bin.display())));

                std::process::exit(status.code().unwrap_or(1));
            }
        }

        "cart" => {
            let name = args.next().unwrap_or_else(|| usage());
            let watch = matches!(args.next().as_deref(), Some("--watch"));
            let root = xtask::repo_root().unwrap_or_else(|e| die(e));
            let out_dir = root.join("target/carts");
            let cart_dir = root.join("carts").join(&name);

            std::fs::create_dir_all(&out_dir).expect("failed to create target/carts");

            match xtask::build_cart(&cart_dir, &out_dir) {
                Ok(v68) => report("Built", &v68),
                Err(e) => die(e),
            }

            if watch {
                let v68 = out_dir.join(format!("{name}.v68"));

                loop {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    let before = mtime(&v68);

                    match xtask::build_cart(&cart_dir, &out_dir) {
                        Ok(v68) if mtime(&v68) != before => report("Built", &v68),
                        Ok(_) => {}
                        Err(e) => eprintln!("{e}"),
                    }
                }
            }
        }

        _ => usage(),
    }
}
