// `CIColorCube`: the colour held on 0…1 and read trilinearly off the cube, red running fastest;
// what the cube returns under zero is clipped, alpha kept.

struct Params { head: vec4<u32> }

// Node (r, g, b) is read through `cube_node(r, g, b, n)`, which each entry point defines.
fn color_cube_px(s: vec4<f32>) -> vec4<f32> {
    let n = P.head.w;
    let f = clamp(s.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * f32(n - 1u);
    let lo = min(vec3<u32>(floor(f)), vec3<u32>(n - 2u));
    let t = f - vec3<f32>(lo);
    let hi = lo + 1u;
    let c00 = mix(cube_node(lo.x, lo.y, lo.z, n), cube_node(hi.x, lo.y, lo.z, n), t.x);
    let c10 = mix(cube_node(lo.x, hi.y, lo.z, n), cube_node(hi.x, hi.y, lo.z, n), t.x);
    let c01 = mix(cube_node(lo.x, lo.y, hi.z, n), cube_node(hi.x, lo.y, hi.z, n), t.x);
    let c11 = mix(cube_node(lo.x, hi.y, hi.z, n), cube_node(hi.x, hi.y, hi.z, n), t.x);
    let c = mix(mix(c00, c10, t.y), mix(c01, c11, t.y), t.z);
    return vec4<f32>(max(c, vec3<f32>(0.0)), s.a);
}
