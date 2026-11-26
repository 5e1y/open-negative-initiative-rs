// The welcome's stage: four wrapped gifts on a black floor, each a box, its lid, its ribbon and bow, and the
// film's plate above the largest; signed distances in the stage's units, a gift's half-width its `place.w`.

struct Gift {
    // xyz where it stands on the floor, w its half-width (0: absent).
    place: vec4<f32>,
    // x its turn about the vertical, y its lift off the floor, z its material, w the light it stands in.
    pose: vec4<f32>,
    // xyz the lid's flight from its seat, in the gift's own units, w its tumble about the x axis.
    lid: vec4<f32>,
    // x the bow, y the lid's bands, each 1 tied to 0 gone; z the body's bands, 1 tied, 0.5 fallen open on the
    // floor, 0 gone; w the light from inside.
    tie: vec4<f32>,
    // xyz the bow's flight from its knot, in the gift's own units, w its tumble about the x axis.
    bow: vec4<f32>,
    // x the body's half-height in half-widths; y how far its walls have fallen open, 0 a box to 1 a cross flat on
    // the floor; z the light leaking under its lid; w how high its light has climbed, 0 to 1.
    shape: vec4<f32>,
}

struct Frame {
    // xy the picture's size, z the time in seconds, w the exposure, 0 black to 1.
    view: vec4<f32>,
    // xyz the camera, w its focal length in picture heights.
    eye: vec4<f32>,
    // xyz what the camera looks at, w the pool of light's radius about it.
    aim: vec4<f32>,
    // x the key light, y the softbox overhead, z the rim behind, w the floor's mirror.
    light: vec4<f32>,
    // xyz the middle of the film's frame, w its half-height (0: no film).
    plate: vec4<f32>,
    // x the plate's turn about the vertical, y the print's sheet, 0 away to 1 there, z its own light, w the grain.
    turn: vec4<f32>,
    // x the scanner's bar across the frame, y the film inverted, 0 a negative to 1 its print, z the lamp behind,
    // w the strip of film, 0 away to 1 there.
    film: vec4<f32>,
    // x and z how far the lens looks off its axis, across and down, in picture heights; y the photo laid back over
    // the print up to that share of it (0: none); w the words' column beside the stage, in picture heights.
    lens: vec4<f32>,
    // x which print shows, a whole number a stock and the share past it the next one coming over it; y 1 while
    // the grain still shimmers; z the pass, 1 the one that draws edges again; w the footer, in picture heights.
    show: vec4<f32>,
    // x how far the strip has still to run, in frame half-heights; y the film developed, 0 to 1; z the latent
    // picture the exposure left; w the halation about its lights.
    strip: vec4<f32>,
    // x the three prints fanned out, 0 to 1; z the camera close on the grain, where the film is seen brighter;
    // w where a strip of light stands round the stage, in radians (0: away).
    fan: vec4<f32>,
    gifts: array<Gift, 4>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var target_picture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var photo: texture_2d<f32>;
@group(0) @binding(3) var negative: texture_2d<f32>;
@group(0) @binding(4) var print: texture_2d<f32>;
@group(0) @binding(5) var smooth_sampler: sampler;
@group(0) @binding(6) var drawn: texture_2d<f32>;

// The box's proportions, in half-widths: the lid's half-height, its overhang, the wall, the round.
const LID: f32 = 0.2;
const OVER: f32 = 0.06;
const WALL: f32 = 0.05;
const ROUND: f32 = 0.032;
// The ribbon: its half-width and how far it stands off the paper.
const BAND: f32 = 0.16;
const PROUD: f32 = 0.008;
// What a hit is made of: the floor, a gift's foil, its ribbon, the inner face of a fallen wall, its lid.
const FLOOR: f32 = 0.0;
const FOIL: f32 = 1.0;
const RIBBON: f32 = 2.0;
const INSIDE: f32 = 3.0;
const LIDFOIL: f32 = 5.0;
const FAR: f32 = 60.0;

fn turned(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y);
}

fn round_box(p: vec3<f32>, half: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec3<f32>(r);
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

// The nearer of two hits, each a distance and what it is made of.
fn nearer(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    if a.x < b.x {
        return a;
    }
    return b;
}

// A point of the stage in a gift's own space: its foot at the origin, its half-width 1.
fn in_gift(p: vec3<f32>, g: Gift) -> vec3<f32> {
    var q = (p - g.place.xyz - vec3<f32>(0.0, g.pose.y, 0.0)) / g.place.w;
    let xz = turned(q.xz, -g.pose.x);
    return vec3<f32>(xz.x, q.y, xz.y);
}

// A gift's point in its lid's space: the lid's middle at the origin, flown and tumbled.
fn in_lid(q: vec3<f32>, g: Gift) -> vec3<f32> {
    var l = q - vec3<f32>(0.0, 2.0 * g.shape.x + LID - 0.08, 0.0) - g.lid.xyz;
    let yz = turned(l.yz, -g.lid.w);
    return vec3<f32>(l.x, yz.x, yz.y);
}

// One loop of the bow: a thin band bent into a drop that swells away from the knot and leans toward the eye;
// pulled, it draws through the knot to nothing.
fn bow_loop(p: vec3<f32>, side: f32, full: f32) -> f32 {
    var l = vec3<f32>(p.x * side, p.y, p.z);
    let xz = turned(l.xz, 0.42);
    l = vec3<f32>(xz.x, l.y, xz.y);
    let xy = turned(l.xy, -0.62 * full);
    l = vec3<f32>(xy.x - 0.44 * full, xy.y, l.z);
    // Narrow at the knot, full at its far end.
    let swell = mix(0.45, 1.0, smoothstep(-0.44, 0.15, l.x)) * full;
    let ring = length(vec2<f32>(l.x * 0.7, l.y / max(swell, 0.01))) - 0.3 * full;
    let band = max(abs(ring) - 0.006, abs(l.z) - BAND * (0.62 + 0.5 * full));
    return band * 0.7 - 0.006;
}

// A tail of the bow: a band running out from the knot and down over the lid's edge, its end cut aslant; it
// lengthens as its loop is drawn through, then narrows away.
fn bow_tail(p: vec3<f32>, side: f32, full: f32, kept: f32) -> f32 {
    var l = vec3<f32>(p.x * side, p.y, p.z);
    let xz = turned(l.xz, -0.75);
    l = vec3<f32>(xz.x, l.y, xz.y);
    let reach = 1.3 + 0.5 * (1.0 - full);
    // Past the lid's edge it hangs.
    let droop = 0.06 * smoothstep(0.2, 0.9, l.x) + 0.4 * smoothstep(0.98, 1.3, l.x);
    let band = round_box(l - vec3<f32>(reach * 0.5, 0.008 - droop, 0.0), vec3<f32>(reach * 0.5, 0.006, BAND * 0.82 * kept), 0.004);
    return max(band, (l.x + l.z * 0.7 - reach * 0.92) * 0.7);
}

// The bow, in the lid's space: two loops, the knot, two tails. `tie.x` unties it: from 1 to 0.3 its loops draw
// through the knot, from 0.3 to 0 what is left narrows away.
fn bow(l: vec3<f32>, g: Gift) -> f32 {
    if g.tie.x <= 0.001 {
        return FAR;
    }
    // Drawn out a little past 1 before it gives.
    let full = smoothstep(0.3, 1.0, min(g.tie.x, 1.0)) * max(g.tie.x, 1.0);
    let kept = smoothstep(0.0, 0.3, g.tie.x);
    var p = l - vec3<f32>(0.0, LID + PROUD + 0.02, 0.0) - g.bow.xyz;
    let yz = turned(p.yz, -g.bow.w);
    p = vec3<f32>(p.x, yz.x, yz.y);
    var d = min(bow_tail(p, -1.0, full, kept), bow_tail(p, 1.0, full, kept));
    if full > 0.02 {
        d = min(d, min(bow_loop(p, -1.0, full), bow_loop(p, 1.0, full)));
    }
    let knot = round_box(p - vec3<f32>(0.0, 0.07, 0.0), vec3<f32>(0.13, 0.09, BAND * 0.95) * kept, 0.05 * kept);
    return min(d, knot);
}

// One of the body's four bands, on the side `k` quarter turns round: hinged at the floor, it falls open and
// lies flat, then narrows to nothing.
fn band(q: vec3<f32>, g: Gift, k: i32) -> f32 {
    let tall = g.shape.x;
    let side = turned(q.xz, -1.5708 * f32(k));
    // Each side lets go a little after the one before.
    let fallen = smoothstep(0.0, 1.0, clamp((1.0 - g.tie.z) * 2.6 - 0.2 * f32(k), 0.0, 1.0));
    var hinge = turned(vec2<f32>(side.x - 1.0 - PROUD, q.y), fallen * 1.5708);
    // Cloth, not card: it bellies out as it falls and lies down from its foot first.
    hinge.x += 0.55 * tall * sin(3.1416 * clamp(hinge.y / (2.0 * tall), 0.0, 1.0)) * fallen * (1.0 - fallen) * 2.4;
    // Lying on the floor it keeps a wave, and its end curls up.
    hinge.x -= (0.035 * sin(hinge.y * 5.5 + f32(k)) + 0.16 * pow(clamp(hinge.y / (2.0 * tall), 0.0, 1.0), 6.0)) * fallen;
    let wide = BAND * smoothstep(0.0, 0.4, g.tie.z);
    return round_box(vec3<f32>(hinge.x, hinge.y - tall, side.y), vec3<f32>(0.007, tall, wide), 0.005) * 0.6;
}

// A gift in its own space: the hollow body, the lid, the bands about both, the bow, the light inside.
fn gift(q: vec3<f32>, g: Gift, index: f32) -> vec2<f32> {
    let tag = 10.0 * index;
    let tall = g.shape.x;
    let b = q - vec3<f32>(0.0, tall, 0.0);
    let shell = round_box(b, vec3<f32>(1.0, tall, 1.0), ROUND);
    let hollow = round_box(b - vec3<f32>(0.0, WALL * 2.0, 0.0), vec3<f32>(1.0 - WALL, tall, 1.0 - WALL), ROUND * 0.5);
    var body = max(shell, -hollow);
    var skin = FOIL;
    if g.shape.y > 0.001 {
        // Its walls fall open about their feet and lie flat about its floor, their inner faces up.
        body = round_box(q - vec3<f32>(0.0, WALL, 0.0), vec3<f32>(1.0, WALL, 1.0), ROUND * 0.5);
        if q.y > WALL {
            skin = INSIDE;
        }
        for (var k = 0; k < 4; k++) {
            let side = turned(q.xz, -1.5708 * f32(k));
            let hinge = turned(vec2<f32>(side.x - 1.0, q.y), g.shape.y * 1.5708);
            let wall = round_box(vec3<f32>(hinge.x + WALL, hinge.y - tall, side.y), vec3<f32>(WALL, tall, 1.0), ROUND * 0.5);
            if wall < body {
                body = wall;
                skin = mix(FOIL, INSIDE, step(hinge.x, -WALL));
            }
        }
    }
    var hit = vec2<f32>(body, tag + skin);
    if g.tie.z > 0.001 {
        var bands = FAR;
        for (var k = 0; k < 4; k++) {
            bands = min(bands, band(q, g, k));
        }
        hit = nearer(hit, vec2<f32>(max(bands, -q.y), tag + RIBBON));
    }
    let l = in_lid(q, g);
    let cap = round_box(l, vec3<f32>(1.0 + OVER, LID, 1.0 + OVER), ROUND);
    let under = round_box(l + vec3<f32>(0.0, WALL * 2.0, 0.0), vec3<f32>(1.0 + OVER - WALL, LID, 1.0 + OVER - WALL), ROUND * 0.5);
    hit = nearer(hit, vec2<f32>(max(cap, -under), tag + LIDFOIL));
    // The lid's bands draw back from the knot to its edges.
    if g.tie.y > 0.001 {
        let wrap = round_box(l, vec3<f32>(1.0 + OVER + PROUD, LID + PROUD, 1.0 + OVER + PROUD), ROUND + PROUD);
        let cross = min(abs(l.x), abs(l.z)) - BAND;
        let drawn = (1.0 + OVER + PROUD) * (1.0 - g.tie.y) - max(abs(l.x), abs(l.z));
        hit = nearer(hit, vec2<f32>(max(max(wrap, cross), max(drawn, -cap)), tag + RIBBON));
    }
    return nearer(hit, vec2<f32>(bow(l, g), tag + RIBBON));
}

// One gift by its number: its distance from `p` and what it is made of.
fn solid(i: i32, p: vec3<f32>) -> vec2<f32> {
    let g = frame.gifts[i];
    let near = gift(in_gift(p, g), g, f32(i));
    return vec2<f32>(near.x * g.place.w, near.y);
}

fn normal_of(i: i32, p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(1.0, -1.0) * 0.0012;
    return normalize(e.xyy * solid(i, p + e.xyy).x + e.yyx * solid(i, p + e.yyx).x + e.yxy * solid(i, p + e.yxy).x + e.xxx * solid(i, p + e.xxx).x);
}

// Where a ray meets a ball: its way in and its way out, the way in past the way out when it misses.
fn ball(start: vec3<f32>, along: vec3<f32>, middle: vec3<f32>, radius: f32) -> vec2<f32> {
    let to = start - middle;
    let b = dot(to, along);
    let h = b * b - dot(to, to) + radius * radius;
    if h < 0.0 {
        return vec2<f32>(FAR, -FAR);
    }
    let root = sqrt(h);
    return vec2<f32>(-b - root, -b + root);
}

// The stretch of a ray in which a gift may stand: a ball about its box and bow, one about its flown lid.
fn reach_of(i: i32, start: vec3<f32>, along: vec3<f32>) -> vec2<f32> {
    let g = frame.gifts[i];
    if g.place.w <= 0.0 || g.pose.w <= 0.0 {
        return vec2<f32>(FAR, -FAR);
    }
    let foot = g.place.xyz + vec3<f32>(0.0, g.pose.y, 0.0);
    // Wider while the bands lie open on the floor or the bow flies.
    var about = 1.9 + 0.5 * g.shape.x;
    if g.tie.z < 0.999 && g.tie.z > 0.001 || dot(g.bow.xyz, g.bow.xyz) > 0.01 || g.shape.y > 0.001 {
        about = 2.2 + 2.2 * g.shape.x;
    }
    var reach = ball(start, along, foot + vec3<f32>(0.0, g.shape.x + 0.3, 0.0) * g.place.w, about * g.place.w);
    if dot(g.lid.xyz, g.lid.xyz) > 0.01 {
        let flown = vec3<f32>(turned(g.lid.xz, g.pose.x).x, g.lid.y + 2.0 * g.shape.x + LID, turned(g.lid.xz, g.pose.x).y);
        let lid = ball(start, along, foot + flown * g.place.w, 1.65 * g.place.w);
        if lid.x < lid.y {
            reach = vec2<f32>(min(reach.x, lid.x), max(reach.y, lid.y));
        }
    }
    return reach;
}

// The first surface of the gifts along a ray, short of `limit`: its distance, what it is made of, which
// thing it is; nothing: a distance of `FAR`.
fn first_solid(start: vec3<f32>, along: vec3<f32>, limit: f32, steps: i32) -> vec3<f32> {
    var best = vec3<f32>(FAR, -1.0, -1.0);
    for (var i = 0; i < 4; i++) {
        let reach = reach_of(i, start, along);
        let end = min(min(reach.y, limit), best.x);
        var t = max(reach.x, 0.01);
        for (var k = 0; k < steps && t < end; k++) {
            let hit = solid(i, start + along * t);
            if hit.x < 0.0008 * t {
                best = vec3<f32>(t, hit.y, f32(i));
                break;
            }
            t += hit.x;
        }
    }
    return best;
}

// How much of a light a point of a thing sees past the thing itself, softened by how near the ray passes it.
fn shadow(i: i32, start: vec3<f32>, toward: vec3<f32>) -> f32 {
    var seen = 1.0;
    var t = 0.04;
    for (var k = 0; k < 18; k++) {
        let d = solid(i, start + toward * t).x;
        seen = min(seen, 9.0 * d / t);
        t += clamp(d, 0.04, 0.6);
        if seen < 0.005 || t > 6.0 {
            break;
        }
    }
    return clamp(seen, 0.0, 1.0);
}

// How open the room about a point of a thing is, from four steps along its normal.
fn open_air(i: i32, p: vec3<f32>, n: vec3<f32>) -> f32 {
    var closed = 0.0;
    var weight = 1.0;
    for (var k = 1; k <= 4; k++) {
        let h = 0.06 * f32(k);
        closed += (h - solid(i, p + n * h).x) * weight;
        weight *= 0.7;
    }
    return clamp(1.0 - 2.2 * closed, 0.0, 1.0);
}

// How much of the room a point of the floor sees: dark under each gift and about its foot, lighter as it lifts.
fn floor_air(p: vec3<f32>) -> f32 {
    var air = 1.0;
    for (var i = 0; i < 4; i++) {
        let g = frame.gifts[i];
        if g.place.w <= 0.0 || g.pose.w <= 0.0 {
            continue;
        }
        let q = turned(p.xz - g.place.xz, -g.pose.x) / g.place.w;
        let away = length(max(abs(q) - vec2<f32>(1.0), vec2<f32>(0.0)));
        let lifted = g.pose.y / g.place.w;
        air *= 1.0 - 0.9 * exp(-away * (3.2 - min(lifted, 2.0))) / (1.0 + 2.5 * lifted);
        // A tight dark line where it meets the floor.
        air *= 1.0 - 0.85 * exp(-away * 22.0) / (1.0 + 30.0 * lifted);
    }
    return air;
}
