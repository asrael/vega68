fn usage() -> ! {
    eprintln!("usage: cargo xtask <bios | cart <name>>");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| usage());

    match cmd.as_str() {
        "bios" => match xtask::burn_rom() {
            Ok(rom) => {
                let bytes = std::fs::metadata(&rom).map(|m| m.len()).unwrap_or(0);
                println!("burned {} ({bytes} bytes)", rom.display());
            }

            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },

        "cart" => {
            let name = args.next().unwrap_or_else(|| usage());
            let root = xtask::repo_root().unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            let out_dir = root.join("target/carts");
            let cart_dir = root.join("carts").join(&name);

            std::fs::create_dir_all(&out_dir).expect("failed to create target/carts");

            match xtask::build_cart(&cart_dir, &out_dir) {
                Ok(v68) => {
                    let bytes = std::fs::metadata(&v68).map(|m| m.len()).unwrap_or(0);
                    println!("built {} ({bytes} bytes)", v68.display());
                }

                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }

        _ => usage(),
    }
}
