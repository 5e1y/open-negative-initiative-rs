// Stages 4 to 7 in one pass, operation for operation as `cpu::stage47` and the Metal kernel,
// so a difference between them is one of precision, never of formula.

struct Params {
    gain_invert: vec4<f32>,
    red_density: vec4<f32>,
    green_pedestal: vec4<f32>,
    blue_guard: vec4<f32>,
    low_window: vec4<f32>,
    high_print: vec4<f32>,
    print_contrast_hunt: vec4<f32>,
    own_base_stops: vec4<f32>,
    balance_low_base_contrast: vec4<f32>,
    balance_span_decompression: vec4<f32>,
    print_zones_base_zones: vec4<f32>,
    film_follow: vec4<f32>,
    own_contrast: vec4<f32>,
    own_mid: vec4<f32>,
    count: vec4<u32>,
}

const CONTRAST_STEP: f32 = 0.05;

fn apply_levels(e: f32, lv: vec3<f32>) -> f32 {
    let n = max((e - lv.x) / max(lv.y - lv.x, 1e-4), 0.0);
    return pow(n, lv.z);
}

fn apply_window(e: f32, lv: vec3<f32>, c1: f32, c3: f32) -> f32 {
    let t = apply_levels(e, lv);
    if (c1 == 0.25 && c3 == 0.75) { return t; }
    let u = min(t, 1.0);
    let v = 1.0 - u;
    let curve = c1 * (4.0 * u * v * v * v) + 0.5 * (6.0 * u * u * v * v)
              + c3 * (4.0 * u * u * u * v) + u * u * u * u;
    return curve + (t - u);
}

fn quarter_pass(p: f32, t: f32, high: bool) -> f32 {
    if (high) { return p + t * 4.0 * p * p * p * (1.0 - p); }
    return p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p);
}

fn quartered(p: f32, travel: f32, high: bool) -> f32 {
    if (abs(travel) <= 0.25) { return quarter_pass(p, travel, high); }
    let first = select(-0.25, 0.25, travel > 0.0);
    return quarter_pass(quarter_pass(p, first, high), travel - first, high);
}

fn shaped(p_in: f32, stops: f32, contrast: f32, zones: vec2<f32>) -> f32 {
    let held = clamp(p_in, 0.0, 1.0);
    var p = contrasted(held, contrast) + (p_in - held);
    if (stops != 0.0 && p > 0.0) {
        let top_ = min(p, 1.0);
        let g = exp2(stops);
        let v = pow(top_, DISPLAY_GAMMA);
        p = pow(v * g / (1.0 + v * (g - 1.0)), 1.0 / DISPLAY_GAMMA) + (p - top_);
    }
    if (any(zones != vec2<f32>(0.0)) && p >= 0.0 && p <= 1.0) {
        p = quartered(p, zones.x, false);
        p = quartered(p, zones.y, true);
    }
    return p;
}

fn printed_rgb(n: f32, stops: f32, contrast: f32, zones: vec2<f32>) -> f32 {
    return pow(max(shaped(n, stops, contrast, zones), 0.0), DISPLAY_GAMMA);
}

fn midded(p: f32, mids: f32) -> f32 {
    if (mids == 0.0 || p < 0.0 || p > 1.0) { return p; }
    return p + mids * 16.0 * p * p * (1.0 - p) * (1.0 - p);
}

fn printed(n: f32, stops: f32, contrast: f32, zones: vec2<f32>, own: f32, own_contrast: f32, own_mid: f32) -> f32 {
    var p = shaped(n, stops, contrast, zones);
    p = midded(shaped(p, own, own_contrast, vec2<f32>(0.0)), own_mid);
    return pow(max(p, 0.0), DISPLAY_GAMMA);
}

// Stages 4 and 5: the film's density, its windows and the chroma decompression.
fn stage47_windowed(s: vec4<f32>) -> vec3<f32> {
    let gain = P.gain_invert.xyz;
    let invert = P.gain_invert.w;
    let red = P.red_density.xyz;
    let green = P.green_pedestal.xyz;
    let pedestal = P.green_pedestal.w;
    let blue = P.blue_guard.xyz;
    let log_guard = P.blue_guard.w;
    let low_points = P.low_window.xyz;
    let window = P.low_window.w;
    let high_points = P.high_print.xyz;
    let balance_low = P.balance_low_base_contrast.xyz;
    let balance_span = P.balance_span_decompression.xyz;
    let decompression = P.balance_span_decompression.w;
    let film_share = P.film_follow.x;

    let t = (s.rgb + pedestal) * gain;
    let d = -log(max(t, vec3<f32>(log_guard))) * LOG10_E;
    var e = select(window - d, d, invert > 0.5);

    let film = e;
    e = vec3<f32>(apply_window(e.r, red, low_points.r, high_points.r),
                  apply_window(e.g, green, low_points.g, high_points.g),
                  apply_window(e.b, blue, low_points.b, high_points.b));
    if (decompression != 1.0 || film_share != 0.0) {
        let place = (film - balance_low) / balance_span;
        let neutral = balance_low + (place.r + place.g + place.b) / 3.0 * balance_span;
        let toned = vec3<f32>(apply_window(neutral.r, red, low_points.r, high_points.r),
                              apply_window(neutral.g, green, low_points.g, high_points.g),
                              apply_window(neutral.b, blue, low_points.b, high_points.b));
        let held = toned + decompression * (e - toned) + film_share * (film - neutral);
        e = held + (dot(e, LUMA) - dot(held, LUMA));
    }
    return e;
}

// Stages 6 and 7 on a windowed pixel: the print, its filters, Hunt and the colour density.
fn stage47_printed(windowed_in: vec3<f32>) -> vec3<f32> {
    var density = P.red_density.w;
    let print_on = P.high_print.w;
    let print_stops = P.print_contrast_hunt.x;
    let print_contrast = P.print_contrast_hunt.y;
    let hunt = P.print_contrast_hunt.z;
    let own = P.own_base_stops.xyz;
    let own_contrast = P.own_contrast.xyz;
    let own_mid = P.own_mid.xyz;
    let base_stops = P.own_base_stops.w;
    let base_contrast = P.balance_low_base_contrast.w;
    let print_zones = P.print_zones_base_zones.xy;
    let base_zones = P.print_zones_base_zones.zw;
    let follow = P.film_follow.y;
    var e = windowed_in;
    if (print_on > 0.5) {
        let windowed = e;
        e = vec3<f32>(printed(e.r, print_stops, print_contrast, print_zones, own.r, own_contrast.r, own_mid.r),
                      printed(e.g, print_stops, print_contrast, print_zones, own.g, own_contrast.g, own_mid.g),
                      printed(e.b, print_stops, print_contrast, print_zones, own.b, own_contrast.b, own_mid.b));
        if (any(own != vec3<f32>(0.0))) {
            let plain = vec3<f32>(printed(windowed.r, print_stops, print_contrast, print_zones, 0.0, own_contrast.r, own_mid.r),
                                  printed(windowed.g, print_stops, print_contrast, print_zones, 0.0, own_contrast.g, own_mid.g),
                                  printed(windowed.b, print_stops, print_contrast, print_zones, 0.0, own_contrast.b, own_mid.b));
            let filtered = dot(e, LUMA);
            if (filtered > 1e-6) { e = e * (dot(plain, LUMA) / filtered); }
        }
        let bent = print_stops != base_stops || print_contrast != base_contrast
                   || any(print_zones != base_zones);
        if (hunt > 0.0) {
            let g = (windowed.r + windowed.g + windowed.b) / 3.0;
            density = density * exp(-hunt * (shaped(g, print_stops, print_contrast, print_zones) - g));
        }
        if (follow < 1.0 && bent) {
            let g = (windowed.r + windowed.g + windowed.b) / 3.0;
            if (g > 1e-3) {
                let up = g * (1.0 + CONTRAST_STEP);
                let down = g * (1.0 - CONTRAST_STEP);
                let curve = log(pow(max(shaped(up, print_stops, print_contrast, print_zones), 1e-6), DISPLAY_GAMMA)
                              / pow(max(shaped(down, print_stops, print_contrast, print_zones), 1e-6), DISPLAY_GAMMA));
                let base = log(pow(max(shaped(up, base_stops, base_contrast, base_zones), 1e-6), DISPLAY_GAMMA)
                             / pow(max(shaped(down, base_stops, base_contrast, base_zones), 1e-6), DISPLAY_GAMMA));
                let k = select(1.0, clamp(curve / base, 0.25, 4.0), abs(base) > 1e-6);
                let grey = vec3<f32>(printed_rgb(g, print_stops, print_contrast, print_zones));
                let held = max(grey + (e - grey) * min(pow(k, follow - 1.0), 1.0), vec3<f32>(0.0));
                let lit = dot(held, LUMA);
                if (lit > 1e-6) { e = held * (dot(e, LUMA) / lit); }
            }
        }
    }

    if (density != 1.0) {
        let pw = pow(max(e, vec3<f32>(0.0)), vec3<f32>(density));
        let p = dot(pw, LUMA);
        if (p > 1e-12) { e = pw * pow(p, (1.0 / density) - 1.0); }
    }
    return e;
}

fn stage47_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(stage47_printed(stage47_windowed(s)), s.a);
}

