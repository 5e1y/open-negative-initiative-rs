// Stages 4 to 7 in one pass, operation for operation as `cpu::stage47` and the Metal kernel,
// so a difference between them is one of precision, never of formula.

struct Params {
    gain_invert: vec4<f32>,
    red: vec4<f32>,
    green_pedestal: vec4<f32>,
    blue_guard: vec4<f32>,
    low_window: vec4<f32>,
    high_print: vec4<f32>,
    print_hunt_paper_film: vec4<f32>,
    balance_low: vec4<f32>,
    balance_span_decompression: vec4<f32>,
    count: vec4<u32>,
}

// The window's share of density `e`, through its mid's power over the black, straight on under it.
fn apply_levels(e: f32, lv: vec3<f32>) -> f32 {
    let n = (e - lv.x) / max(lv.y - lv.x, 1e-4);
    return select(n, pow(max(n, 0.0), lv.z), n > 0.0);
}

fn apply_window(e: f32, lv: vec3<f32>, c1: f32, c3: f32) -> f32 {
    let t = apply_levels(e, lv);
    if (c1 == 0.25 && c3 == 0.75) { return t; }
    let u = clamp(t, 0.0, 1.0);
    let v = 1.0 - u;
    let curve = c1 * (4.0 * u * v * v * v) + 0.5 * (6.0 * u * u * v * v)
              + c3 * (4.0 * u * u * u * v) + u * u * u * u;
    return curve + (t - u);
}

// A share carried past 0 to 1 straight on: a share past 2 rounds to a neighbour, which every pinned render keeps.
fn carried(p: f32) -> f32 {
    let held = clamp(p, 0.0, 1.0);
    return held + (p - held);
}

// ONI's print curve, its stops on the display value under 1, straight on past it.
fn shaped(p_in: f32, stops: f32) -> f32 {
    var p = carried(p_in);
    if (stops != 0.0 && p > 0.0) {
        let top_ = min(p, 1.0);
        let g = exp2(stops);
        let v = pow(top_, DISPLAY_GAMMA);
        p = pow(v * g / (1.0 + v * (g - 1.0)), 1.0 / DISPLAY_GAMMA) + (p - top_);
    }
    return p;
}

fn printed(n: f32, stops: f32) -> f32 {
    return pow(max(carried(shaped(n, stops)), 0.0), DISPLAY_GAMMA);
}

// Stage 4: each channel's density, turned over on a positive.
fn stage47_density(s: vec4<f32>) -> vec3<f32> {
    let t = (s.rgb + P.green_pedestal.w) * P.gain_invert.xyz;
    let d = -log(max(t, vec3<f32>(P.blue_guard.w))) * LOG10_E;
    return select(P.low_window.w - d, d, P.gain_invert.w > 0.5);
}

// Stage 5: each channel's density read through its window, the share both prints read.
fn stage47_window(e: vec3<f32>) -> vec3<f32> {
    let low_points = P.low_window.xyz;
    let high_points = P.high_print.xyz;
    return vec3<f32>(apply_window(e.r, P.red.xyz, low_points.r, high_points.r),
                     apply_window(e.g, P.green_pedestal.xyz, low_points.g, high_points.g),
                     apply_window(e.b, P.blue_guard.xyz, low_points.b, high_points.b));
}

// ONI's print's chroma decompression on the window's shares of the density `film`: the windows' colour and the film's
// own, the luma held.
fn stage47_decompressed(film: vec3<f32>, e: vec3<f32>) -> vec3<f32> {
    let balance_low = P.balance_low.xyz;
    let balance_span = P.balance_span_decompression.xyz;
    let decompression = P.balance_span_decompression.w;
    let film_share = P.print_hunt_paper_film.w;
    if (decompression == 1.0 && film_share == 0.0) { return e; }
    let place = (film - balance_low) / balance_span;
    let neutral = balance_low + (place.r + place.g + place.b) / 3.0 * balance_span;
    let toned = stage47_window(neutral);
    let held = toned + decompression * (e - toned) + film_share * (film - neutral);
    return held + (dot(e, LUMA) - dot(held, LUMA));
}

// Stages 6 and 7 on a windowed pixel: the print and Hunt's colour.
fn stage47_printed(windowed_in: vec3<f32>) -> vec3<f32> {
    var density = 1.0;
    let print_on = P.high_print.w;
    let print_stops = P.print_hunt_paper_film.x;
    let hunt = P.print_hunt_paper_film.y;
    var e = windowed_in;
    if (print_on > 0.5) {
        let windowed = e;
        e = vec3<f32>(printed(e.r, print_stops), printed(e.g, print_stops), printed(e.b, print_stops));
        if (hunt > 0.0) {
            let g = (windowed.r + windowed.g + windowed.b) / 3.0;
            density = density * exp(-hunt * (shaped(g, print_stops) - g));
        }
    }

    if (density != 1.0) {
        let pw = pow(max(e, vec3<f32>(0.0)), vec3<f32>(density));
        let p = dot(pw, LUMA);
        if (p > 1e-12) { e = pw * pow(p, (1.0 / density) - 1.0); }
    }
    return e;
}

// The window's shares go on to the paper's pass where one prints, else through ONI's print.
fn stage47_px(s: vec4<f32>) -> vec4<f32> {
    let film = stage47_density(s);
    let shares = stage47_window(film);
    if (P.print_hunt_paper_film.z > 0.5) { return vec4<f32>(shares, s.a); }
    return vec4<f32>(stage47_printed(stage47_decompressed(film, shares)), s.a);
}
