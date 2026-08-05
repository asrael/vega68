use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

pub const CART_MAX: usize = 0x0100_0000;
pub const HEADER_LEN: usize = 16;

const BIOS_EXPORTS: &[&str] = &["v68_monitor", "v68_reset"];

#[derive(Debug, PartialEq)]
pub struct CartLayout {
    pub sources: Vec<PathBuf>,
}

pub fn bios_section(name: &str) -> Result<(u32, u32, u32), String> {
    let elf = repo_root()?.join("target/bios/vega68.elf");
    let image = std::fs::read(&elf).map_err(|e| format!("{}: {e}", elf.display()))?;
    let be = |at: usize, len: usize| -> Result<u32, String> {
        image
            .get(at..at + len)
            .map(|b| b.iter().fold(0u32, |v, b| (v << 8) | *b as u32))
            .ok_or_else(|| format!("{}: truncated at {at:#x}", elf.display()))
    };

    if !image.starts_with(b"\x7fELF\x01\x02") {
        return Err(format!("{}: not a big-endian elf32 image", elf.display()));
    }

    let shoff = be(0x20, 4)? as usize;
    let shentsize = be(0x2e, 2)? as usize;
    let shnum = be(0x30, 2)? as usize;
    let shstrndx = be(0x32, 2)? as usize;
    let shdr = |i: usize, field: usize| be(shoff + i * shentsize + field, 4);
    let names = shdr(shstrndx, 16)? as usize;

    for i in 0..shnum {
        let tail = image
            .get(names + shdr(i, 0)? as usize..)
            .unwrap_or_default();
        let end = tail.iter().position(|&b| b == 0).unwrap_or(0);

        if &tail[..end] == name.as_bytes() {
            return Ok((shdr(i, 12)?, shdr(i, 4)?, shdr(i, 20)?));
        }
    }

    Err(format!("{}: no section {name}", elf.display()))
}

pub fn bios_symbol(name: &str) -> Result<u32, String> {
    let map = repo_root()?.join("target/bios/vega68.map");
    let text = std::fs::read_to_string(&map).map_err(|e| format!("{}: {e}", map.display()))?;

    map_symbols(&text)
        .find(|(_, sym)| *sym == name)
        .map(|(addr, _)| addr)
        .ok_or_else(|| format!("{}: no symbol {name}", map.display()))
}

pub fn build_bios() -> Result<PathBuf, String> {
    static LOCK: Mutex<()> = Mutex::new(());

    let _held = serialize(&LOCK);

    let root = repo_root()?;
    let bios = root.join("bios");
    let out_dir = root.join("target/bios");
    let elf = out_dir.join("vega68.elf");
    let bin = out_dir.join("vega68.bin");
    let map = out_dir.join("vega68.map");
    let sym = out_dir.join("bios.sym");
    let stamp = out_dir.join("bios.inputs");
    let elf_tmp = tmp_path(&elf);
    let bin_tmp = tmp_path(&bin);
    let map_tmp = tmp_path(&map);

    let mut args = bios_cflags(&root);

    args.push(format!("-Wl,-L,{}", s(&root.join("devkit"))));
    args.push(format!("-Wl,-T,{}", s(&bios.join("bios.ld"))));
    args.push("-Wl,-z,noexecstack".to_owned());
    args.extend(
        ["crt0.s", "main.c", "monitor.c"]
            .iter()
            .map(|f| s(&bios.join(f))),
    );

    let mut inputs = find_files(&bios, &["c", "h", "ld", "s"])?;

    inputs.extend(find_files(&root.join("devkit"), &["h", "ld"])?);

    let want = args.join("\n");

    args.extend([
        format!("-Wl,-Map={}", s(&map_tmp)),
        "-o".to_owned(),
        s(&elf_tmp),
    ]);

    if !stale(&bin, &stamp, &want, &inputs) && map.exists() {
        emit_bios_sym(&map, &sym)?;

        return Ok(bin);
    }

    eprintln!("building bios...");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    run("m68k-elf-gcc", &args, None)?;
    run(
        "m68k-elf-objcopy",
        &["-O", "binary", &s(&elf_tmp), &s(&bin_tmp)],
        None,
    )?;

    let image = std::fs::read(&bin_tmp).map_err(|e| e.to_string())?;

    if image
        .get(0x80..0x400)
        .is_none_or(|pad| pad.iter().any(|&b| b != 0))
    {
        return Err("vega68.bin: code overlaps the vector table pad".into());
    }

    publish(&elf_tmp, &elf)?;
    publish(&map_tmp, &map)?;
    publish(&bin_tmp, &bin)?;
    emit_bios_sym(&map, &sym)?;
    write_atomic(&stamp, want.as_bytes())?;

    Ok(bin)
}

pub fn build_cart(cart_dir: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    static LOCK: Mutex<()> = Mutex::new(());

    let _held = serialize(&LOCK);

    let name = cart_name(cart_dir)?;
    let layout = cart_layout(cart_dir)?;

    build_native_cart(&layout.sources, out_dir, &name)
}

pub fn burn_rom() -> Result<PathBuf, String> {
    let bin = build_bios()?;
    let rom = repo_root()?.join("bios/vega68.rom");
    let image = std::fs::read(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;

    write_atomic(&rom, &image)?;

    Ok(rom)
}

pub fn cart_layout(cart_dir: &Path) -> Result<CartLayout, String> {
    let sources = find_files(cart_dir, &["c"])?;
    let has_main = sources
        .iter()
        .any(|p| p.parent() == Some(cart_dir) && p.file_name() == Some(OsStr::new("main.c")));

    if !has_main {
        return Err(format!(
            "{}: not a cart; it has no main.c",
            cart_dir.display()
        ));
    }

    Ok(CartLayout { sources })
}

pub fn repo_root() -> Result<PathBuf, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().map_err(|e| {
        format!(
            "{}: {e} (this binary was built from a checkout that has since moved or been deleted)",
            root.display()
        )
    })?;

    #[cfg(windows)]
    let root = match root.to_str() {
        Some(s) => PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(s)),
        None => root,
    };

    Ok(root)
}

fn bios_cflags(root: &Path) -> Vec<String> {
    let mut flags: Vec<String> = [
        "-m68040",
        "-Os",
        "-ffreestanding",
        "-nostdlib",
        "-std=c99",
        "-Wall",
    ]
    .map(String::from)
    .into();

    flags.extend(["bios", "devkit"].map(|d| inc(root, d)));

    flags
}

fn build_native_cart(sources: &[PathBuf], out_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let root = repo_root()?;
    let carts = root.join("carts");
    let elf = out_dir.join(format!("{name}.elf"));
    let v68 = out_dir.join(format!("{name}.v68"));
    let v68_tmp = tmp_path(&v68);

    build_bios()?;
    prepare_out(out_dir, &v68)?;

    let mut args = vec![
        "-m68040".to_owned(),
        "-O2".to_owned(),
        "-ffreestanding".to_owned(),
        "-nostdlib".to_owned(),
        "-std=c99".to_owned(),
        "-Wall".to_owned(),
    ];

    args.push(inc(&root, "devkit"));
    args.extend([
        format!("-Wl,-L,{}", s(&root.join("devkit"))),
        format!("-Wl,-T,{}", s(&carts.join("cart.ld"))),
        "-Wl,-z,noexecstack".to_owned(),
        s(&root.join("target/bios/bios.sym")),
        s(&carts.join("crt0.s")),
    ]);
    args.extend(sources.iter().map(|p| s(p)));
    args.extend(["-o".to_owned(), s(&elf)]);

    eprintln!("building cart {name} (native)...");
    run("m68k-elf-gcc", &args, None)?;
    run(
        "m68k-elf-objcopy",
        &["-O", "binary", &s(&elf), &s(&v68_tmp)],
        None,
    )?;

    let image = std::fs::metadata(&v68_tmp).map_err(|e| format!("{}: {e}", v68_tmp.display()))?;

    check_cart_len(image.len() as usize).map_err(|e| format!("{}: {e}", v68.display()))?;
    publish(&v68_tmp, &v68)?;

    Ok(v68)
}

fn cart_name(cart_dir: &Path) -> Result<String, String> {
    cart_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_owned())
        .ok_or_else(|| format!("{}: not a cart directory", cart_dir.display()))
}

fn check_cart_len(len: usize) -> Result<(), String> {
    if len > CART_MAX {
        return Err(format!("{len} bytes, over the {CART_MAX}-byte cart window"));
    }

    Ok(())
}

fn emit_bios_sym(map: &Path, out: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(map).map_err(|e| format!("{}: {e}", map.display()))?;
    let mut lines = Vec::new();
    let mut seen = Vec::new();

    for (addr, name) in map_symbols(&text) {
        if BIOS_EXPORTS.contains(&name) {
            lines.push(format!("PROVIDE({name} = {addr:#010x});"));
            seen.push(name);
        }
    }

    let missing: Vec<&str> = BIOS_EXPORTS
        .iter()
        .copied()
        .filter(|e| !seen.contains(e))
        .collect();

    if !missing.is_empty() {
        return Err(format!(
            "{}: the bios does not export {} (a listed symbol was renamed, \
             became static/inline, or the map format changed)",
            map.display(),
            missing.join(", ")
        ));
    }

    lines.sort();
    lines.dedup();

    write_atomic(out, lines.join("\n").as_bytes())
}

fn find_files(dir: &Path, exts: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();

        if path.is_dir() {
            files.extend(find_files(&path, exts)?);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && exts.contains(&ext)
        {
            files.push(path);
        }
    }

    files.sort();

    Ok(files)
}

fn inc(root: &Path, dir: &str) -> String {
    format!("-I{}", s(&root.join(dir)))
}

fn map_symbols(text: &str) -> impl Iterator<Item = (u32, &str)> {
    text.lines().filter_map(|line| {
        let mut it = line.split_whitespace();
        let hex = it.next()?.strip_prefix("0x")?;
        let name = it.next()?;

        match it.next() {
            None | Some("=") => u32::from_str_radix(hex, 16).ok().map(|a| (a, name)),
            Some(_) => None,
        }
    })
}

fn prepare_out(out_dir: &Path, v68: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;

    if let Err(e) = std::fs::remove_file(v68)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(format!("{}: {e}", v68.display()));
    }

    Ok(())
}

fn publish(tmp: &Path, dst: &Path) -> Result<(), String> {
    std::fs::rename(tmp, dst).map_err(|e| format!("{}: {e}", dst.display()))
}

fn run<S: AsRef<OsStr>>(name: &str, args: &[S], cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(name);
    cmd.args(args);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let out = cmd
        .output()
        .map_err(|e| format!("failed to spawn {name}: {e}"))?;

    if out.status.success() {
        eprint!("{}", String::from_utf8_lossy(&out.stderr));

        Ok(())
    } else {
        Err(format!(
            "{name} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

// each builder takes its own lock and they nest, so one shared lock deadlocks
fn serialize(lock: &'static Mutex<()>) -> MutexGuard<'static, ()> {
    lock.lock().unwrap_or_else(|e| e.into_inner())
}

fn stale(target: &Path, stamp: &Path, want: &str, inputs: &[PathBuf]) -> bool {
    let Ok(built) = std::fs::metadata(target).and_then(|m| m.modified()) else {
        return true;
    };

    if !std::fs::read_to_string(stamp).is_ok_and(|prev| prev == want) {
        return true;
    }

    inputs.iter().any(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t > built)
    })
}

fn tmp_path(p: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    let n = NEXT.fetch_add(1, Ordering::Relaxed);

    p.with_extension(format!("{ext}.{}.{n}.tmp", std::process::id()))
}

fn write_atomic(dst: &Path, body: &[u8]) -> Result<(), String> {
    if std::fs::read(dst).is_ok_and(|prev| prev == body) {
        return Ok(());
    }

    let tmp = tmp_path(dst);

    std::fs::write(&tmp, body).map_err(|e| format!("{}: {e}", tmp.display()))?;

    publish(&tmp, dst)
}
