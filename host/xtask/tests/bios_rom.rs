#[test]
fn burned_rom_matches_the_bios_sources() {
    let sum = xtask::repo_root().unwrap().join("bios/vega68.rom.sum");
    let burned = std::fs::read_to_string(&sum)
        .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask bios`", sum.display()));

    assert!(
        xtask::bios_checksum().unwrap() == burned.trim(),
        "bios/vega68.rom is stale; run `cargo xtask bios`"
    );
}
