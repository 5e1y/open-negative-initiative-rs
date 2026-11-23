// A scanned slide read as its film's dyes: each pixel's light, over the backlight and the bare base, solved by Newton's
// steps for the three amounts whose light through the base and dyes under the scanner's lamp is it; coded as 10^-amount.

struct Params {
    head: vec4<u32>,
    gain: vec4<f32>,
    start: array<vec4<f32>, 3>,
}

// The table is read through `dye_table(i, row)`, which the entry point defines: the dyes and the base, then the lamp's
// light in working Rec. 2020, `head.x` bands each; `head.y` steps.
fn dye_amounts_px(s: vec4<f32>) -> vec4<f32> {
    let light = max(s.rgb * P.gain.rgb, vec3<f32>(1e-6));
    let aim = log2(light) * 0.30102999566;
    var a = clamp(-vec3<f32>(dot(P.start[0].rgb, aim), dot(P.start[1].rgb, aim), dot(P.start[2].rgb, aim)), vec3<f32>(-0.5), vec3<f32>(6.0));
    for (var n = 0u; n < P.head.y; n = n + 1u) {
        var f = vec3<f32>(0.0);
        var j0 = vec3<f32>(0.0);
        var j1 = vec3<f32>(0.0);
        var j2 = vec3<f32>(0.0);
        for (var i = 0u; i < P.head.x; i = i + 1u) {
            let dye = dye_table(i, 0u);
            let seen = dye_table(i, 1u).rgb;
            let t = exp2(-(dye.w + dot(a, dye.rgb)) * 3.32192809489);
            let lit = t * seen;
            f = f + lit;
            j0 = j0 - lit.x * dye.rgb;
            j1 = j1 - lit.y * dye.rgb;
            j2 = j2 - lit.z * dye.rgb;
        }
        let g = max(f, vec3<f32>(1e-12));
        let r0 = j0 / g.x;
        let r1 = j1 / g.y;
        let r2 = j2 / g.z;
        let miss = log2(g) * 0.30102999566 - aim;
        let ca = r1.y * r2.z - r1.z * r2.y;
        let cb = r1.z * r2.x - r1.x * r2.z;
        let cc = r1.x * r2.y - r1.y * r2.x;
        let det = r0.x * ca + r0.y * cb + r0.z * cc;
        if (abs(det) < 1e-30) {
            break;
        }
        let i0 = vec3<f32>(ca, r0.z * r2.y - r0.y * r2.z, r0.y * r1.z - r0.z * r1.y);
        let i1 = vec3<f32>(cb, r0.x * r2.z - r0.z * r2.x, r0.z * r1.x - r0.x * r1.z);
        let i2 = vec3<f32>(cc, r0.y * r2.x - r0.x * r2.y, r0.x * r1.y - r0.y * r1.x);
        let d = vec3<f32>(dot(i0, miss), dot(i1, miss), dot(i2, miss)) / det;
        a = clamp(a - clamp(d, vec3<f32>(-0.5), vec3<f32>(0.5)), vec3<f32>(-0.5), vec3<f32>(6.0));
    }
    return vec4<f32>(exp2(-a * 3.32192809489), s.a);
}
