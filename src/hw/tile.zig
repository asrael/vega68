pub const TilemapEntry = packed struct(u16) {
    hflip: u1 = 0,
    vflip: u1 = 0,

    pallete: u4 = 0,
    tile: u10 = 0,
};
