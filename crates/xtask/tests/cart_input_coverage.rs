use std::fs::{FileTimes, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

static LOCK: Mutex<()> = Mutex::new(());

fn serialize() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn tool_available(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .is_ok()
}

fn gather(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();

        if path.is_dir() {
            files.extend(gather(&path, exts));
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && exts.contains(&ext)
        {
            files.push(path);
        }
    }

    files
}

fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .modified()
        .unwrap()
}

fn set_mtime(path: &Path, t: SystemTime) {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    file.set_times(FileTimes::new().set_modified(t))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn copy_cart_backdated(src: &Path, dst: &Path) {
    let _ = std::fs::remove_dir_all(dst);
    std::fs::create_dir_all(dst).unwrap();

    let ancient = SystemTime::UNIX_EPOCH + Duration::from_secs(1);

    for file in gather(src, &["c", "h", "ld"]) {
        let rel = file.strip_prefix(src).unwrap();
        let out = dst.join(rel);

        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::copy(&file, &out).unwrap();
        set_mtime(&out, ancient);
    }
}

struct MtimeGuard {
    path: PathBuf,
    original: SystemTime,
}

impl Drop for MtimeGuard {
    fn drop(&mut self) {
        if let Ok(file) = OpenOptions::new().write(true).open(&self.path) {
            let _ = file.set_times(FileTimes::new().set_modified(self.original));
        }
    }
}

fn out_dir(case: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(case);

    let _ = std::fs::remove_dir_all(&dir);

    dir
}

#[test]
fn devkit_header_change_forces_a_cart_rebuild() {
    if !tool_available("m68k-elf-gcc") {
        eprintln!("skipping: m68k-elf-gcc not on PATH");
        return;
    }

    let _held = serialize();
    let root = xtask::repo_root().unwrap();
    let out_dir = out_dir("cart_input_coverage_devkit");
    let cart_dir = out_dir.join("cart_src");

    copy_cart_backdated(&root.join("carts/hello"), &cart_dir);
    xtask::build_bios().unwrap();

    let v68 = xtask::build_cart(&cart_dir, &out_dir).unwrap();

    let devkit_hw = root.join("devkit/vega68_hw.h");
    let other_devkit: Vec<PathBuf> = gather(&root.join("devkit"), &["h", "ld"])
        .into_iter()
        .filter(|p| *p != devkit_hw)
        .collect();

    let control_max = [
        mtime(&root.join("carts/cart.ld")),
        mtime(&root.join("carts/crt0.s")),
        mtime(&root.join("target/bios/bios.sym")),
    ]
    .into_iter()
    .chain(other_devkit.iter().map(|p| mtime(p)))
    .max()
    .unwrap();

    let threshold = control_max + Duration::from_secs(1);
    let bumped = threshold + Duration::from_secs(1);
    let guard = MtimeGuard {
        path: devkit_hw.clone(),
        original: mtime(&devkit_hw),
    };

    set_mtime(&devkit_hw, bumped);
    set_mtime(&v68, threshold);

    let v68_again = xtask::build_cart(&cart_dir, &out_dir).unwrap();
    let rebuilt = mtime(&v68_again);

    drop(guard);

    assert_ne!(
        rebuilt, threshold,
        "build_cart did not rebuild a cart with a newer devkit/vega68_hw.h — \
         the devkit glob dropped out of build_cart's tracked input set"
    );
}

#[test]
fn bios_sym_change_forces_a_cart_rebuild() {
    if !tool_available("m68k-elf-gcc") {
        eprintln!("skipping: m68k-elf-gcc not on PATH");
        return;
    }

    let _held = serialize();
    let root = xtask::repo_root().unwrap();
    let out_dir = out_dir("cart_input_coverage_bios");
    let cart_dir = out_dir.join("cart_src");

    copy_cart_backdated(&root.join("carts/hello"), &cart_dir);
    xtask::build_bios().unwrap();

    let v68 = xtask::build_cart(&cart_dir, &out_dir).unwrap();

    let bios_sym = root.join("target/bios/bios.sym");
    let devkit_files = gather(&root.join("devkit"), &["h", "ld"]);

    let control_max = [
        mtime(&root.join("carts/cart.ld")),
        mtime(&root.join("carts/crt0.s")),
    ]
    .into_iter()
    .chain(devkit_files.iter().map(|p| mtime(p)))
    .max()
    .unwrap();

    let threshold = control_max + Duration::from_secs(1);
    let bumped = threshold + Duration::from_secs(1);
    let guard = MtimeGuard {
        path: bios_sym.clone(),
        original: mtime(&bios_sym),
    };

    set_mtime(&bios_sym, bumped);
    set_mtime(&v68, threshold);

    let v68_again = xtask::build_cart(&cart_dir, &out_dir).unwrap();
    let rebuilt = mtime(&v68_again);

    drop(guard);

    assert_ne!(
        rebuilt, threshold,
        "build_cart did not rebuild a cart with a newer target/bios/bios.sym — \
         bios.sym dropped out of build_cart's tracked input set"
    );
}
