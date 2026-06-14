export fn memcmp(a: [*]const u8, b: [*]const u8, n: usize) callconv(.c) c_int {
    for (0..n) |i| {
        if (a[i] != b[i]) return @as(c_int, a[i]) - @as(c_int, b[i]);
    }
    return 0;
}

export fn memcpy(noalias dest: [*]u8, noalias src: [*]const u8, n: usize) callconv(.c) [*]u8 {
    for (0..n) |i| dest[i] = src[i];
    return dest;
}

export fn memmove(dest: [*]u8, src: [*]const u8, n: usize) callconv(.c) [*]u8 {
    if (@intFromPtr(dest) < @intFromPtr(src)) {
        for (0..n) |i| dest[i] = src[i];
    } else {
        var i: usize = n;
        while (i > 0) {
            i -= 1;
            dest[i] = src[i];
        }
    }
    return dest;
}

export fn memset(dest: [*]u8, c: c_int, n: usize) callconv(.c) [*]u8 {
    const byte: u8 = @truncate(@as(u32, @bitCast(c)));
    for (0..n) |i| dest[i] = byte;
    return dest;
}
