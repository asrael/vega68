//! vega68 zig build script
//!
//!  zig build      -> emulator exe (vega68) + asset tool (vega68-pack)
//!  zig build run  -> run the emulator
//!  zig build test -> compile-check the native tree + run unit tests
//!  zig build rom  -> freestanding m68k rom object
//!
//! > [!NOTE]
//! > freestanding rom target requires a zig/llvm built with m68k backend

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const hw_mod = b.createModule(.{
        .root_source_file = b.path("src/hw/hw.zig"),
    });

    const core_mod = b.createModule(.{
        .root_source_file = b.path("src/core/system.zig"),
        .target = target,
        .optimize = optimize,
    });
    core_mod.addImport("hw", hw_mod);

    const shell_mod = b.createModule(.{
        .root_source_file = b.path("src/shell/shell.zig"),
        .target = target,
        .optimize = optimize,
    });
    shell_mod.addImport("core", core_mod);

    const pack_mod = b.createModule(.{
        .root_source_file = b.path("tools/pack/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    pack_mod.addImport("hw", hw_mod);

    const exe_mod = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    exe_mod.addImport("core", core_mod);
    exe_mod.addImport("shell", shell_mod);

    const exe = b.addExecutable(.{ .name = "vega68", .root_module = exe_mod });
    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    run_cmd.setCwd(b.path(".")); // run from repo root (ROM path is repo-relative)
    if (b.args) |args| run_cmd.addArgs(args);
    const run_step = b.step("run", "Load + run a ROM on the vega68 emulator (default: sprite)");
    run_step.dependOn(&run_cmd.step);

    const tool = b.addExecutable(.{ .name = "vega68-pack", .root_module = pack_mod });
    b.installArtifact(tool);

    const test_step = b.step("test", "Compile-check the native tree and run unit tests");

    // each module compile-checks itself
    for ([_]*std.Build.Module{ core_mod, shell_mod, pack_mod }) |mod| {
        test_step.dependOn(&b.addRunArtifact(b.addTest(.{ .root_module = mod })).step);
    }

    const bus_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/tests/bus_test.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    bus_test.root_module.addImport("core", core_mod);
    test_step.dependOn(&b.addRunArtifact(bus_test).step);

    const win32_shell_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/tests/test_win32_shell.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    win32_shell_test.root_module.addImport("core", core_mod);
    win32_shell_test.root_module.addImport("shell", shell_mod);
    test_step.dependOn(&b.addRunArtifact(win32_shell_test).step);

    const ea_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/tests/ea_test.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    ea_test.root_module.addImport("core", core_mod);
    test_step.dependOn(&b.addRunArtifact(ea_test).step);

    const mame_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/tests/mame_test.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    mame_test.root_module.addImport("core", core_mod);
    test_step.dependOn(&b.addRunArtifact(mame_test).step);

    const musashi_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/tests/musashi_test.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    musashi_test.root_module.addImport("core", core_mod);
    test_step.dependOn(&b.addRunArtifact(musashi_test).step);

    const exception_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/tests/exception_test.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    exception_test.root_module.addImport("core", core_mod);
    test_step.dependOn(&b.addRunArtifact(exception_test).step);

    // devkit fixed-point math has no hw dep; verified natively.
    const math_test = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/devkit/runtime/math.zig"),
        .target = target,
        .optimize = optimize,
    }) });
    test_step.dependOn(&b.addRunArtifact(math_test).step);

    // devkit rom; only the `rom` step touches the m68k backend
    {
        const m68k = b.resolveTargetQuery(.{
            .cpu_arch = .m68k,
            .os_tag = .freestanding,
        });

        // BIOS/kernel image (vos.vro)
        const bios_bin = blk: {
            const bios_mod = b.createModule(.{
                .root_source_file = b.path("src/devkit/os/os.zig"),
                .target = m68k,
                .optimize = .ReleaseSmall,
                .code_model = .large,
            });
            bios_mod.addImport("hw", hw_mod);

            const bios_obj = b.addObject(.{ .name = "bios", .root_module = bios_mod });

            const bios_ld = b.addSystemCommand(&.{ "m68k-elf-ld", "-T" });
            bios_ld.addFileArg(b.path("src/devkit/bios.ld"));
            bios_ld.addArg("-o");
            const bios_elf = bios_ld.addOutputFileArg("bios.elf");
            bios_ld.addFileArg(bios_obj.getEmittedBin());

            const bios_oc = b.addSystemCommand(&.{ "m68k-elf-objcopy", "-O", "binary" });
            bios_oc.addFileArg(bios_elf);
            const vro = bios_oc.addOutputFileArg("vos.vro");

            const install_bios = b.addInstallBinFile(vro, "vos.vro");
            // Off the default install; requires m68k cross-linker. Pulled in by `zig build run`.
            run_cmd.step.dependOn(&install_bios.step);
            break :blk vro;
        };

        const devkit_mod = b.createModule(.{
            .root_source_file = b.path("src/devkit/devkit.zig"),
            .target = m68k,
            .optimize = .ReleaseSmall,
            .code_model = .large,
        });
        devkit_mod.addImport("hw", hw_mod);

        const example_hello_mod = b.createModule(.{
            .root_source_file = b.path("src/devkit/examples/hello/main.zig"),
            .target = m68k,
            .optimize = .ReleaseSmall,
            .code_model = .large,
        });
        example_hello_mod.addImport("devkit", devkit_mod);

        const rom_obj = b.addObject(.{ .name = "hello", .root_module = example_hello_mod });

        // link: hello.o + cart.ld -> hello.elf (user cartridge at $00400000, with CartHeader)
        const ld = b.addSystemCommand(&.{ "m68k-elf-ld", "-T" });
        ld.addFileArg(b.path("src/devkit/cart.ld"));
        ld.addArg("-o");
        const elf = ld.addOutputFileArg("hello.elf");
        ld.addFileArg(rom_obj.getEmittedBin());

        // flatten: hello.elf -> hello.vro
        const oc = b.addSystemCommand(&.{ "m68k-elf-objcopy", "-O", "binary" });
        oc.addFileArg(elf);
        const bin = oc.addOutputFileArg("hello.vro");

        const install_bin = b.addInstallBinFile(bin, "hello.vro");
        const rom_step = b.step("rom", "Build the freestanding m68k ROM images (hello + sprite)");
        rom_step.dependOn(&install_bin.step);

        run_cmd.step.dependOn(&install_bin.step);

        // sprite example cart: a d-pad-controllable sprite (the default `run` target).
        const sprite_mod = b.createModule(.{
            .root_source_file = b.path("src/devkit/examples/sprite/main.zig"),
            .target = m68k,
            .optimize = .ReleaseSmall,
            .code_model = .large,
        });
        sprite_mod.addImport("devkit", devkit_mod);

        const sprite_obj = b.addObject(.{ .name = "sprite", .root_module = sprite_mod });

        const sprite_ld = b.addSystemCommand(&.{ "m68k-elf-ld", "-T" });
        sprite_ld.addFileArg(b.path("src/devkit/cart.ld"));
        sprite_ld.addArg("-o");
        const sprite_elf = sprite_ld.addOutputFileArg("sprite.elf");
        sprite_ld.addFileArg(sprite_obj.getEmittedBin());

        const sprite_oc = b.addSystemCommand(&.{ "m68k-elf-objcopy", "-O", "binary" });
        sprite_oc.addFileArg(sprite_elf);
        const sprite_bin = sprite_oc.addOutputFileArg("sprite.vro");

        const install_sprite = b.addInstallBinFile(sprite_bin, "sprite.vro");
        rom_step.dependOn(&install_sprite.step);
        run_cmd.step.dependOn(&install_sprite.step);

        // abitest cart: drives the firmware ABI and signals pass/fail via exit status.
        const abitest_mod = b.createModule(.{
            .root_source_file = b.path("src/devkit/examples/abitest/main.zig"),
            .target = m68k,
            .optimize = .ReleaseSmall,
            .code_model = .large,
        });
        abitest_mod.addImport("devkit", devkit_mod);

        const abitest_obj = b.addObject(.{ .name = "abitest", .root_module = abitest_mod });

        const abitest_ld = b.addSystemCommand(&.{ "m68k-elf-ld", "-T" });
        abitest_ld.addFileArg(b.path("src/devkit/cart.ld"));
        abitest_ld.addArg("-o");
        const abitest_elf = abitest_ld.addOutputFileArg("abitest.elf");
        abitest_ld.addFileArg(abitest_obj.getEmittedBin());

        const abitest_oc = b.addSystemCommand(&.{ "m68k-elf-objcopy", "-O", "binary" });
        abitest_oc.addFileArg(abitest_elf);
        const abitest_bin = abitest_oc.addOutputFileArg("abitest.vro");

        // host-run conformance test: embeds the real BIOS + abitest cart and runs
        // them on the core. Gated (own step) since it needs the m68k cross toolchain.
        const abi_test_mod = b.createModule(.{
            .root_source_file = b.path("src/tests/syscall_test.zig"),
            .target = target,
            .optimize = optimize,
        });
        abi_test_mod.addImport("core", core_mod);
        abi_test_mod.addAnonymousImport("vos.vro", .{ .root_source_file = bios_bin });
        abi_test_mod.addAnonymousImport("abitest.vro", .{ .root_source_file = abitest_bin });

        const abi_test_step = b.step("test-abi", "Run the on-device ABI conformance test (needs m68k toolchain)");
        abi_test_step.dependOn(&b.addRunArtifact(b.addTest(.{ .root_module = abi_test_mod })).step);

        // host-run video/input test: embeds the real BIOS + sprite cart, drives input,
        // and asserts the sprite renders + moves. Gated (needs the m68k cross toolchain).
        const video_test_mod = b.createModule(.{
            .root_source_file = b.path("src/tests/video_test.zig"),
            .target = target,
            .optimize = optimize,
        });
        video_test_mod.addImport("core", core_mod);
        video_test_mod.addAnonymousImport("vos.vro", .{ .root_source_file = bios_bin });
        video_test_mod.addAnonymousImport("sprite.vro", .{ .root_source_file = sprite_bin });

        const video_test_step = b.step("test-video", "Run the on-device video/input test (needs m68k toolchain)");
        video_test_step.dependOn(&b.addRunArtifact(b.addTest(.{ .root_module = video_test_mod })).step);
    }

    // cpu-vectors: clones upstream musashi, builds m68kmake + core, runs vectorgen at repo root.
    {
        const host = b.resolveTargetQuery(.{});
        const cpu_vectors_step = b.step("cpu-vectors", "regen musashi cpu test vectors (clones upstream musashi)");

        const clone = b.addSystemCommand(&.{ "git", "clone", "--depth", "1", "https://github.com/kstenerud/Musashi.git" });
        const musashi = clone.addOutputDirectoryArg("musashi");
        clone.has_side_effects = true; // re-clone each run to track upstream HEAD

        // m68kmake codegen: m68k_in.c -> m68kops.{h,c}
        const mk_mod = b.createModule(.{ .target = host, .optimize = .ReleaseFast, .link_libc = true });
        mk_mod.addCSourceFile(.{ .file = musashi.path(b, "m68kmake.c") });
        const m68kmake = b.addExecutable(.{ .name = "m68kmake", .root_module = mk_mod });

        const run_mk = b.addRunArtifact(m68kmake);
        const gen_dir = run_mk.addOutputDirectoryArg("m68kgen"); // argv[1]: output dir
        run_mk.addFileArg(musashi.path(b, "m68k_in.c")); // argv[2]: input template

        const gen_mod = b.createModule(.{
            .root_source_file = b.path("tools/vectorgen/main.zig"),
            .target = host,
            .optimize = .ReleaseFast,
            .link_libc = true,
        });
        gen_mod.addCSourceFiles(.{
            .root = musashi,
            .files = &.{ "m68kcpu.c", "softfloat/softfloat.c" },
            .flags = &.{"-fno-sanitize=undefined"},
        });
        gen_mod.addCSourceFile(.{ .file = gen_dir.path(b, "m68kops.c"), .flags = &.{"-fno-sanitize=undefined"} });
        gen_mod.addIncludePath(musashi);
        gen_mod.addIncludePath(gen_dir);
        const vectorgen = b.addExecutable(.{ .name = "vectorgen", .root_module = gen_mod });

        const run_gen = b.addRunArtifact(vectorgen);
        run_gen.setCwd(b.path(".")); // repo root: main.zig uses repo-relative paths
        run_gen.has_side_effects = true; // writes into the source tree
        cpu_vectors_step.dependOn(&run_gen.step);
    }
}
