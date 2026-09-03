use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let musashi = PathBuf::from("../musashi");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=../musashi");

    let compiler = cc::Build::new().get_compiler();
    let m68kmake = out.join(format!("m68kmake{}", env::consts::EXE_SUFFIX));
    let mut build_generator = compiler.to_command();

    if compiler.is_like_msvc() {
        build_generator
            .arg(musashi.join("m68kmake.c"))
            .arg(format!("/Fe{}", m68kmake.display()))
            .arg(format!("/Fo{}\\", out.display()));
    } else {
        build_generator
            .arg(musashi.join("m68kmake.c"))
            .arg("-o")
            .arg(&m68kmake);
    }

    run(&mut build_generator);
    run(Command::new(&m68kmake)
        .arg(&out)
        .arg(musashi.join("m68k_in.c")));

    cc::Build::new()
        .include(&musashi)
        .include(&out)
        .file(musashi.join("m68kcpu.c"))
        .file(musashi.join("m68kdasm.c"))
        .file(musashi.join("softfloat/softfloat.c"))
        .file(out.join("m68kops.c"))
        .opt_level(2)
        .warnings(false)
        .compile("musashi");

    if env::var_os("CARGO_CFG_UNIX").is_some() {
        println!("cargo:rustc-link-lib=m");
    }
}

fn run(cmd: &mut Command) {
    let status = cmd.status().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));

    assert!(status.success(), "{cmd:?}: {status}");
}
