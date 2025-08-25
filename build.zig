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
    }) });
    wasm.entry = .disabled;
    wasm.use_llvm = false; // disable llvm backend
    wasm.use_lld = false; // disable llvm linker
    wasm.root_module.export_symbol_names = &.{"add_two_numbers"};

    b.installArtifact(wasm);
}
