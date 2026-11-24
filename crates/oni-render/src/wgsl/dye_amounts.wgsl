// A scanned slide read as its film's dyes: each pixel's light, as the observer sees it over the backlight and the bare
// base, solved by damped Gauss-Newton steps for the amounts whose light under the scanner's lamp is it; 10^-amount out.

struct Params {
    head: vec4<u32>,
    gain: vec4<f32>,
    to_seen: array<vec4<f32>, 3>,
    start: array<vec4<f32>, 3>,
}

struct DyeMissed {
    miss: vec3<f32>,
    // The Jacobian's columns, one per dye.
    c0: vec3<f32>,
    c1: vec3<f32>,
    c2: vec3<f32>,
}

// The table is read through `dye_table(i, row)`, which the entry point defines: the dyes and the base, then the lamp's
// light as the observer sees it, `head.x` bands each; `head.y` steps after the first guess.
fn dye_missed(a: vec3<f32>, aim: vec3<f32>) -> DyeMissed {
    var f = vec3<f32>(0.0);
    var c0 = vec3<f32>(0.0);
    var c1 = vec3<f32>(0.0);
    var c2 = vec3<f32>(0.0);
    for (var i = 0u; i < P.head.x; i = i + 1u) {
        let dye = dye_table(i, 0u);
        let seen = dye_table(i, 1u).rgb;
        let lit = exp2(-(dye.w + dot(a, dye.rgb)) * 3.32192809489) * seen;
        f = f + lit;
        c0 = c0 - lit * dye.x;
        c1 = c1 - lit * dye.y;
        c2 = c2 - lit * dye.z;
    }
    let g = max(f, vec3<f32>(1e-12));
    return DyeMissed(log2(g) * 0.30102999566 - aim, c0 / g, c1 / g, c2 / g);
}

// The squared miss's gradient, halved: the Jacobian's transpose on the miss.
fn dye_slope(m: DyeMissed) -> vec3<f32> {
    return vec3<f32>(dot(m.c0, m.miss), dot(m.c1, m.miss), dot(m.c2, m.miss));
}

fn dye_amounts_px(s: vec4<f32>) -> vec4<f32> {
    let light = s.rgb * P.gain.rgb;
    let seen = max(vec3<f32>(dot(P.to_seen[0].rgb, light), dot(P.to_seen[1].rgb, light), dot(P.to_seen[2].rgb, light)), vec3<f32>(1e-6));
    let aim = log2(seen) * 0.30102999566;
    var a = clamp(-vec3<f32>(dot(P.start[0].rgb, aim), dot(P.start[1].rgb, aim), dot(P.start[2].rgb, aim)), vec3<f32>(-0.5), vec3<f32>(6.0));
    var m = dye_missed(a, aim);
    var damping = P.gain.w;
    for (var n = 0u; n < P.head.y; n = n + 1u) {
        // The normal equations, damped on their diagonal, symmetric; an amount on a bound the descent pushes past is
        // held there, the others solved without it.
        let slope = dye_slope(m);
        let held = ((a <= vec3<f32>(-0.5)) & (slope > vec3<f32>(0.0))) | ((a >= vec3<f32>(6.0)) & (slope < vec3<f32>(0.0)));
        let free = select(vec3<f32>(1.0), vec3<f32>(0.0), held);
        let n00 = select(dot(m.c0, m.c0) * (1.0 + damping), 1.0, held.x);
        let n11 = select(dot(m.c1, m.c1) * (1.0 + damping), 1.0, held.y);
        let n22 = select(dot(m.c2, m.c2) * (1.0 + damping), 1.0, held.z);
        let n01 = dot(m.c0, m.c1) * free.x * free.y;
        let n02 = dot(m.c0, m.c2) * free.x * free.z;
        let n12 = dot(m.c1, m.c2) * free.y * free.z;
        let gr = slope * free;
        let ca = n11 * n22 - n12 * n12;
        let cb = n12 * n02 - n01 * n22;
        let cc = n01 * n12 - n11 * n02;
        let det = n00 * ca + n01 * cb + n02 * cc;
        if (abs(det) < 1e-30) {
            break;
        }
        let i0 = vec3<f32>(ca, n02 * n12 - n01 * n22, n01 * n12 - n02 * n11);
        let i1 = vec3<f32>(cb, n00 * n22 - n02 * n02, n02 * n01 - n00 * n12);
        let i2 = vec3<f32>(cc, n01 * n02 - n00 * n12, n00 * n11 - n01 * n01);
        let step = vec3<f32>(dot(i0, gr), dot(i1, gr), dot(i2, gr)) / det;
        let next = clamp(a - step, vec3<f32>(-0.5), vec3<f32>(6.0));
        let tried = dye_missed(next, aim);
        // Kept where it misses less, or where its slopes at both ends say so: near the root the misses differ under f32's
        // grain, while the slopes' trapezoid still reads the fall.
        let falling = dot(slope + dye_slope(tried), a - next) > 0.0;
        if (dot(tried.miss, tried.miss) < dot(m.miss, m.miss) || falling) {
            a = next;
            m = tried;
            damping = damping * 0.1;
        } else {
            damping = damping * 10.0;
        }
    }
    return vec4<f32>(exp2(-a * 3.32192809489), s.a);
}
