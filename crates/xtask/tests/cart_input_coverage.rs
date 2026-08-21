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

    for file in xtask::find_files(src, &["c", "h", "ld"]).unwrap() {
        let rel = file.strip_prefix(src).unwrap();
        let out = dst.join(rel);

        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::copy(&file, &out).unwrap();
        set_mtime(&out, ancient);
    }
}

struct MtimeGuard {
    original: SystemTime,
    path: PathBuf,
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

fn bump_forces_rebuild(case: &str, bumped_rel: &str, diagnosis: &str) {
    if !tool_available("m68k-elf-gcc") {
        eprintln!("skipping: m68k-elf-gcc not on PATH");
        return;
    }

    let _held = serialize();
    let root = xtask::repo_root().unwrap();
    let out_dir = out_dir(case);
    let cart_dir = out_dir.join("cart_src");

    copy_cart_backdated(&root.join("carts/hello"), &cart_dir);
    xtask::build_bios().unwrap();

    let v68 = xtask::build_cart(&cart_dir, &out_dir).unwrap();

    let bumped = root.join(bumped_rel);
    let control_max = [
        root.join("carts/cart.ld"),
        root.join("carts/crt0.s"),
        root.join("target/bios/bios.sym"),
    ]
    .into_iter()
    .chain(xtask::find_files(&root.join("devkit"), &["c", "h", "ld"]).unwrap())
    .filter(|p| *p != bumped)
    .map(|p| mtime(&p))
    .max()
    .unwrap();

    let threshold = control_max + Duration::from_secs(1);
    let guard = MtimeGuard {
        original: mtime(&bumped),
        path: bumped.clone(),
    };

    set_mtime(&bumped, threshold + Duration::from_secs(1));
    set_mtime(&v68, threshold);

    let v68_again = xtask::build_cart(&cart_dir, &out_dir).unwrap();
    let rebuilt = mtime(&v68_again);

    drop(guard);

    assert_ne!(rebuilt, threshold, "{diagnosis}");
}

#[test]
fn devkit_header_change_forces_a_cart_rebuild() {
    bump_forces_rebuild(
        "cart_input_coverage_devkit",
        "devkit/vega68_hw.h",
        "build_cart did not rebuild a cart with a newer devkit/vega68_hw.h — \
         the devkit glob dropped out of build_cart's tracked input set",
    );
}

#[test]
fn bios_sym_change_forces_a_cart_rebuild() {
    bump_forces_rebuild(
        "cart_input_coverage_bios",
        "target/bios/bios.sym",
        "build_cart did not rebuild a cart with a newer target/bios/bios.sym — \
         bios.sym dropped out of build_cart's tracked input set",
    );
}

#[test]
fn devkit_source_change_forces_a_cart_rebuild() {
    bump_forces_rebuild(
        "cart_input_coverage_devkit_src",
        "devkit/vega68_sfx.c",
        "build_cart did not rebuild a cart with a newer devkit/vega68_sfx.c — \
         the devkit source glob dropped out of build_cart's tracked input set",
    );
}
