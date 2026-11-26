// The picture of the stage, in light before any curve: the camera's rays marched to the gifts, the floor's
// mirror, the light that pours out of an opened box, the film's plate; then its edges drawn again, finer.

// What a surface is, to tell it from its neighbours: its matter, and a share that tells one face from the next.
fn named(what: f32, n: vec3<f32>) -> f32 {
    return what + 0.45 * (0.5 + 0.5 * dot(n, vec3<f32>(0.36, 0.48, 0.8)));
}

// A gift's surface at `p`, and what it is.
fn surface(p: vec3<f32>, v: vec3<f32>, what: f32, thing: i32) -> vec4<f32> {
    let n = normal_of(thing, p);
    return vec4<f32>(gift_colour(p, n, v, what, thing), named(what, n));
}

// What a ray meets first, as `seen_along` names it, with nothing lit.
fn met(start: vec3<f32>, along: vec3<f32>) -> f32 {
    var ground = FAR;
    if along.y < -0.0001 {
        ground = -start.y / along.y;
    }
    let hit = first_solid(start, along, ground, 72);
    if plate_reach(start, along) < min(min(hit.x, 40.0), ground) {
        return 300.0;
    }
    if hit.y > 0.0 {
        return named(hit.y, normal_of(i32(hit.z), start + along * hit.x));
    }
    return 100.0 * step(ground, FAR - 1.0);
}

// A noise that leaves no pattern across neighbouring pixels.
fn scattered(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(0.06711056 * pixel.x + 0.00583715 * pixel.y));
}

// The light standing over an opened box, summed along the ray as far as `reach`: a cone from the box's mouth
// that thins as it climbs and is gone by `HIGH` half-widths, less while it still rises; its colour the foil's.
const HIGH: f32 = 7.5;

fn beams(start: vec3<f32>, along: vec3<f32>, reach: f32, pixel: vec2<f32>) -> vec3<f32> {
    var lit = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let g = frame.gifts[i];
        let high = HIGH * g.shape.w;
        if g.place.w <= 0.0 || g.tie.w <= 0.0 || g.pose.w <= 0.0 || high <= 0.01 {
            continue;
        }
        let s = g.place.w;
        let mouth = g.place.xyz + vec3<f32>(0.0, g.pose.y + 2.0 * g.shape.x * (1.0 - g.shape.y) * s, 0.0);
        // The stretch of the ray inside the upright tube that holds the whole cone.
        let widest = (0.8 + 0.13 * HIGH) * s * 1.6;
        let to = start - mouth;
        let a = dot(along.xz, along.xz);
        let b = dot(to.xz, along.xz);
        let h = b * b - a * (dot(to.xz, to.xz) - widest * widest);
        if h <= 0.0 || a < 0.000001 {
            continue;
        }
        let root = sqrt(h);
        let enter = max((-b - root) / a, 0.0);
        let leave = min((-b + root) / a, reach);
        if leave <= enter {
            continue;
        }
        var sum = vec3<f32>(0.0);
        let shift = scattered(pixel + f32(i) * 13.0);
        for (var k = 0; k < 16; k++) {
            let t = mix(enter, leave, (f32(k) + shift) / 16.0);
            let p = start + along * t - mouth;
            let height = p.y / s;
            if height < -0.3 || height > high {
                continue;
            }
            let wide = (0.8 + 0.13 * max(height, 0.0)) * s;
            let off = length(p.xz) / wide;
            // Burning at the mouth, thinning as it climbs to nothing, its edge soft, rays of dust turning in it.
            let climb = (0.12 + 3.2 * exp(-max(height, 0.0) * 1.25)) * smoothstep(high, high * 0.3, height) * smoothstep(-0.3, 0.15, height);
            let rays = 0.7 + 0.6 * noise(vec2<f32>(atan2(p.z, p.x) * 3.0 + frame.view.z * 0.08, height * 0.35 - frame.view.z * 0.15));
            sum += beam_colour(i32(g.pose.z), p.x / s * 0.22 + height * 0.06) * exp(-off * off * 2.6) * climb * rays;
        }
        lit += sum / 16.0 * (leave - enter) / s * 0.42 * g.tie.w * g.pose.w;
    }
    return lit;
}

// The camera's ray through a pixel: the eye, the aim, the lens.
fn ray(pixel: vec2<f32>) -> vec3<f32> {
    let forward = normalize(frame.aim.xyz - frame.eye.xyz);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let at = (pixel - 0.5 * frame.view.xy) / frame.view.y;
    return normalize(forward * frame.eye.w + right * (at.x - frame.lens.x) - up * (at.y - frame.lens.z));
}

// What a ray sees, and what it is: the floor is where the ray meets it, the gifts marched where their balls stand.
fn seen_along(start: vec3<f32>, along: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    var ground = FAR;
    if along.y < -0.0001 {
        ground = -start.y / along.y;
    }
    let hit = first_solid(start, along, ground, 72);
    var seen = vec4<f32>(0.0);
    var reach = min(hit.x, 40.0);
    if hit.y > 0.0 {
        seen = surface(start + along * hit.x, -along, hit.y, i32(hit.z));
    } else if ground < FAR {
        let p = start + along * ground;
        reach = ground;
        // The floor's far reach goes dark, so no horizon shows.
        seen = vec4<f32>(floor_colour(p) * smoothstep(50.0, 24.0, ground), 100.0);
        // The floor's mirror: the foot of what stands on it, gone a little way up.
        let down = reflect(along, vec3<f32>(0.0, 1.0, 0.0));
        let below = first_solid(p, down, 6.0, 40);
        let fresnel = 0.07 + 0.5 * pow(1.0 - max(-along.y, 0.0), 4.0);
        if below.y > 0.0 {
            let q = p + down * below.x;
            let mirrored = surface(q, -down, below.y, i32(below.z));
            seen = vec4<f32>(seen.rgb + mirrored.rgb * fresnel * frame.light.w * exp(-q.y * 4.2), 100.0);
        }
    }
    let plate = plate_reach(start, along);
    if plate < reach {
        // The box's light stands behind the film, never before it.
        let behind = seen.rgb + beams(start, along, reach, pixel);
        return vec4<f32>(plate_over(start + along * plate, along, plate, behind), 300.0);
    }
    return vec4<f32>(seen.rgb + beams(start, along, reach, pixel), seen.a);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<u32>(frame.view.xy);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let pixel = vec2<f32>(id.xy) + 0.5;
    let at = vec2<i32>(id.xy);
    if frame.show.z < 0.5 {
        let seen = seen_along(frame.eye.xyz, ray(pixel), pixel);
        textureStore(target_picture, at, vec4<f32>(seen.rgb * frame.view.w, seen.a));
        return;
    }
    // The second pass: where a pixel's neighbour is another face or another thing, three more rays say how much
    // of the pixel each one covers, and each brings the colour of the neighbour it lands on.
    let last = vec2<i32>(size) - 1;
    let here = textureLoad(drawn, at, 0);
    var about: array<vec4<f32>, 8>;
    var step = 0.0;
    for (var k = 0; k < 8; k++) {
        let j = k + k / 4;
        about[k] = textureLoad(drawn, clamp(at + vec2<i32>(j % 3 - 1, j / 3 - 1), vec2<i32>(0), last), 0);
        step = max(step, abs(about[k].a - here.a));
    }
    var colour = here.rgb;
    if step > 0.035 {
        var offs = array<vec2<f32>, 3>(vec2<f32>(0.25, 0.375), vec2<f32>(-0.375, 0.125), vec2<f32>(0.125, -0.375));
        for (var k = 0; k < 3; k++) {
            let name = met(frame.eye.xyz, ray(pixel + offs[k]));
            var like = here;
            for (var j = 0; j < 8; j++) {
                if abs(about[j].a - name) < abs(like.a - name) {
                    like = about[j];
                }
            }
            colour += like.rgb;
        }
        colour /= 4.0;
    }
    // The film's pictures throw no glow: they are shown as their files hold them.
    textureStore(target_picture, at, vec4<f32>(colour, select(1.0, 0.0, here.a > 250.0)));
}
