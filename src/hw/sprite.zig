pub const Sprite = packed struct(u64) {
    y: u10 = 0,
    x: u10 = 0,
    w: u2 = 0,
    h: u2 = 0,
    enable: u1 = 0,
    tile: u11 = 0,
    hflip: u1 = 0,
    vflip: u1 = 0,
    palette: u4 = 0,
    prio: u2 = 0,
    _pad: u20 = 0,
};
