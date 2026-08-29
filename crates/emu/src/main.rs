mod app;
mod sdl;
mod watch;

use vega68::cart;
use vega68::{BIOS, System};

use app::App;
use watch::Watch;

fn error(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!("usage: vega68 <cart.v68> [--headless N] [--scale K] [--watch]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut cart_path = None;
    let mut headless = None;
    let mut scale = None;
    let mut watch = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                headless = Some(
                    args.next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--scale" => {
                scale = Some(
                    args.next()
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or_else(|| usage())
                        .max(1),
                )
            }
            "--watch" => watch = true,
            _ if cart_path.is_none() => cart_path = Some(arg),
            _ => usage(),
        }
    }

    let exe = std::env::current_exe()
        .and_then(std::fs::read)
        .unwrap_or_default();
    let (system, watch) = match cart::bundled(&exe).unwrap_or_else(|e| error(&e.to_string())) {
        Some(bundled) => {
            if cart_path.is_some() || watch {
                error("this binary has a bundled cart; only --headless and --scale apply");
            }

            let system = System::new(BIOS, bundled)
                .unwrap_or_else(|e| error(&format!("bundled cart is invalid: {e}")));

            (system, None)
        }

        None => {
            let cart_path = cart_path.unwrap_or_else(|| usage());
            let file = std::fs::read(&cart_path)
                .unwrap_or_else(|e| error(&format!("failed to read {cart_path}: {e}")));
            let system = System::new(BIOS, &file)
                .unwrap_or_else(|e| error(&format!("{cart_path} is not a valid cart: {e}")));

            (system, watch.then(|| Watch::new(cart_path, file)))
        }
    };

    match headless {
        Some(frames) => App::run_headless(system, frames, watch),
        None => App::new(system, scale, watch).run(),
    }
}
