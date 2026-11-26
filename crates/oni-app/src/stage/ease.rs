//! The curves the stage moves by: every move heavy, leaving slowly and landing slower.

/// How far `t` is through the span from `a` to `b`, 0 before it and 1 after.
pub fn span(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

/// Rest to rest: slow out, slow in.
pub fn glide(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    if p < 0.5 { 16.0 * p.powi(5) } else { 1.0 - (-2.0 * p + 2.0).powi(5) / 2.0 }
}

/// A weight landing: quick off the mark, then a long settle.
pub fn land(p: f32) -> f32 {
    1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(4)
}

/// A weight leaving: slow to start, as something heavy gives way.
pub fn give(p: f32) -> f32 {
    p.clamp(0.0, 1.0).powi(3)
}

pub fn mix(a: f32, b: f32, p: f32) -> f32 {
    a + (b - a) * p
}

pub fn mix3(a: [f32; 3], b: [f32; 3], p: f32) -> [f32; 3] {
    [mix(a[0], b[0], p), mix(a[1], b[1], p), mix(a[2], b[2], p)]
}
