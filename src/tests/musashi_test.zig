//! musashi validation vectors

const h = @import("cpu_vectors.zig");

const musashi_only = [_][]const u8{
    "mulu_l", "muls_l", "divu_l", "divs_l",
};

test "musashi mul.l/div.l vectors" {
    inline for (musashi_only) |f| try h.runFile("vectors/musashi/" ++ f ++ ".json");
}

// regen: `zig build cpu-vectors`
test "musashi 68000 cross-reference vectors" {
    try h.runSet68000("vectors/musashi/", h.Profile.musashi);
}
