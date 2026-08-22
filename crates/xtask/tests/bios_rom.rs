#[test]
fn burned_rom_matches_the_bios_sources() {
    if std::process::Command::new("m68k-elf-gcc")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping: m68k-elf-gcc not on PATH");
        return;
    }

    let built = std::fs::read(xtask::build_bios().unwrap()).unwrap();
    let rom = xtask::repo_root().unwrap().join("bios/vega68.rom");
    let burned = std::fs::read(&rom).unwrap_or_else(|e| panic!("{}: {e}", rom.display()));

    assert!(
        built == burned,
        "bios/vega68.rom is stale; run `cargo xtask bios`"
    );
}
