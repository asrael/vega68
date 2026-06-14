pub const Rect = struct { x: i32, y: i32, w: i32, h: i32 };

pub fn fit(client_w: i32, client_h: i32, src_w: i32, src_h: i32) Rect {
    const s = @max(@min(@divTrunc(client_w, src_w), @divTrunc(client_h, src_h)), 1);
    const out_w = src_w * s;
    const out_h = src_h * s;

    return .{
        .x = @divTrunc(client_w - out_w, 2),
        .y = @divTrunc(client_h - out_h, 2),
        .w = out_w,
        .h = out_h,
    };
}
