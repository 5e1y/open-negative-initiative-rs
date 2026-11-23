// A slide projected: each layer's dye, read off the light its channel passes, filters the lamp wavelength by
// wavelength; the screen's light, adapted to the bare film's, in working Rec. 2020 with its veil; alpha kept.

struct Params {
    head: vec4<u32>,
    to_working: array<vec4<f32>, 3>,
}

// The table is read through `projection_table(i, row)`, which the entry point defines: the dyes and the base, then the
// lamp as seen, `head.x` bands each.
fn projection_px(s: vec4<f32>) -> vec4<f32> {
    let amounts = -log2(max(s.rgb, vec3<f32>(1e-12))) * 0.30102999566;
    var xyz = vec3<f32>(0.0);
    for (var i = 0u; i < P.head.x; i = i + 1u) {
        let dye = projection_table(i, 0u);
        let density = dye.w + dot(amounts, dye.rgb);
        xyz = xyz + exp2(-density * 3.32192809489) * projection_table(i, 1u).rgb;
    }
    let flare = P.to_working[0].w;
    return vec4<f32>(dot(P.to_working[0].rgb, xyz) + flare, dot(P.to_working[1].rgb, xyz) + flare, dot(P.to_working[2].rgb, xyz) + flare, s.a);
}
