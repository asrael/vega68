const Operator = struct {
    total_level: u7 = 0,
};

pub const Voice = struct {
    algorithm: u3 = 0,
    ops: [4]Operator = @splat(.{}),

    pub fn render(self: *Voice) i16 {
        _ = self;

        @panic("TODO: 4-operator FM sample");
    }
};
