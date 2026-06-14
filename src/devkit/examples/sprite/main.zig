const sdk = @import("devkit");

const Button = sdk.input.Button;

const clamp = sdk.math.clamp;
const commit = sdk.sprites.commit;
const pressed = sdk.input.pressed;
const vblank = sdk.sync.waitForVblank;

export fn main() void {
    sdk.palette.set(0, 0x101820);
    sdk.palette.set(1, 0xFB4934);

    var x: i32 = 152;
    var y: i32 = 112;

    while (true) {
        if (pressed(0, Button.left)) x -= 2;
        if (pressed(0, Button.right)) x += 2;
        if (pressed(0, Button.up)) y -= 2;
        if (pressed(0, Button.down)) y += 2;

        x = clamp(x, 0, 320 - 16);
        y = clamp(y, 0, 240 - 16);

        sdk.sprites.shadow[0] = .{
            .x = @intCast(x),
            .y = @intCast(y),
            .w = 1,
            .h = 1,
            .enable = 1,
            .palette = 0,
        };

        commit();
        vblank();
    }
}
