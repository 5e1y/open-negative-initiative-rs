use bytemuck::{Pod, Zeroable};

/// The fused kernel's arguments, named as the Metal kernel names them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage47 {
    pub gain: [f32; 3],
    pub red: [f32; 3],
    pub green: [f32; 3],
    pub blue: [f32; 3],
    pub invert: f32,
    pub density: f32,
    pub pedestal: f32,
    pub log_guard: f32,
    pub window: f32,
    pub low_points: [f32; 3],
    pub high_points: [f32; 3],
    pub sep_r: [f32; 3],
    pub sep_g: [f32; 3],
    pub sep_b: [f32; 3],
    pub print_on: f32,
    pub print_stops: f32,
    pub print_contrast: f32,
    pub print_zones: [f32; 2],
    pub own_stops: [f32; 3],
    pub hunt: f32,
    pub base_stops: f32,
    pub base_contrast: f32,
    pub base_zones: [f32; 2],
    pub decompression: f32,
    pub balance_low: [f32; 3],
    pub balance_span: [f32; 3],
    pub film_share: f32,
    pub follow: f32,
}

impl Stage47 {
    /// A negative at rest: no print, identity separation, no decompression.
    pub fn rest() -> Self {
        Self {
            gain: [1.0; 3],
            red: [0.0, 2.0, 1.0],
            green: [0.0, 2.0, 1.0],
            blue: [0.0, 2.0, 1.0],
            invert: 1.0,
            density: 1.0,
            pedestal: 0.0,
            log_guard: 1e-6,
            window: 3.0,
            low_points: [0.25; 3],
            high_points: [0.75; 3],
            sep_r: [1.0, 0.0, 0.0],
            sep_g: [0.0, 1.0, 0.0],
            sep_b: [0.0, 0.0, 1.0],
            print_on: 0.0,
            print_stops: 0.0,
            print_contrast: 0.0,
            print_zones: [0.0; 2],
            own_stops: [0.0; 3],
            hunt: 0.0,
            base_stops: 0.0,
            base_contrast: 0.0,
            base_zones: [0.0; 2],
            decompression: 1.0,
            balance_low: [0.0; 3],
            balance_span: [1.0; 3],
            film_share: 0.0,
            follow: 1.0,
        }
    }

    /// The uniform block, every vector padded to four lanes as WGSL lays it out.
    pub fn packed(&self, count: u32) -> Packed {
        let v = |a: [f32; 3], w: f32| [a[0], a[1], a[2], w];
        Packed {
            rows: [
                v(self.gain, self.invert),
                v(self.red, self.density),
                v(self.green, self.pedestal),
                v(self.blue, self.log_guard),
                v(self.low_points, self.window),
                v(self.high_points, self.print_on),
                v(self.sep_r, self.print_stops),
                v(self.sep_g, self.print_contrast),
                v(self.sep_b, self.hunt),
                v(self.own_stops, self.base_stops),
                v(self.balance_low, self.base_contrast),
                v(self.balance_span, self.decompression),
                [self.print_zones[0], self.print_zones[1], self.base_zones[0], self.base_zones[1]],
                [self.film_share, self.follow, 0.0, 0.0],
            ],
            count: [count, 0, 0, 0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Packed {
    rows: [[f32; 4]; 14],
    count: [u32; 4],
}
