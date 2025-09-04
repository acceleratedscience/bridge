const std = @import("std");

pub fn build(b: *std.Build) void {
    // make target wasm
    const target = b.resolveTargetQuery(.{
        .cpu_arch = .wasm32,
        .os_tag = .freestanding,
    });
    const optimize = b.standardOptimizeOption(.{});

    const wasm = b.addExecutable(.{ .name = "bridge", .root_module = b.createModule(.{
        .target = target,
        .root_source_file = b.path("static/wasm/lib.zig"),
        .optimize = optimize,
        .strip = b.option(bool, "strip", "Strip debug symbols") orelse false,
    }) });
    wasm.entry = .disabled;
    wasm.use_llvm = false; // disable llvm backend
    wasm.use_lld = false; // disable llvm linker
    wasm.root_module.export_symbol_names = &.{"add_two_numbers"};

    b.installArtifact(wasm);

    // test
    const test_wasm = b.addTest(.{ .root_module = b.createModule(.{
        .target = b.standardTargetOptions(.{}),
        .root_source_file = b.path("static/wasm/lib.zig"),
        .optimize = optimize,
    }) });
    const test_wasm_run = b.addRunArtifact(test_wasm);

    const test_step = b.step("test", "Test wasm lib");
    test_step.dependOn(&test_wasm_run.step);
}
