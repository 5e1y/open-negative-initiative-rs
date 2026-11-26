// The stage's light and matter: a black studio lit by softboxes, the design system's foils (`Foil`'s ramps
// and the holographic seal's spectrum) as brushed metal, the satin of the ribbons, the floor's mirror.

fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c / 255.0, vec3<f32>(2.2));
}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

// A softbox seen along `d`: a lit rectangle on the plane a unit away along `axis`, its edges `soft` wide.
fn softbox(d: vec3<f32>, axis: vec3<f32>, across: vec3<f32>, middle: vec2<f32>, half: vec2<f32>, soft: f32) -> f32 {
    let facing = dot(d, axis);
    if facing <= 0.02 {
        return 0.0;
    }
    let up = cross(axis, across);
    let at = vec2<f32>(dot(d, across), dot(d, up)) / facing - middle;
    let edge = smoothstep(half + vec2<f32>(soft), half - vec2<f32>(soft), abs(at));
    return edge.x * edge.y;
}

// The studio a metal mirrors: two tall strips before the stage, one each side, whose bands cross the faces as a
// gift or the camera turns; a wide low box, a box overhead, a long strip low behind for the lids' tops.
fn studio(d: vec3<f32>, soft: f32) -> f32 {
    let drift = 0.18 * sin(frame.view.z * 0.35);
    let over = softbox(d, vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec2<f32>(drift, -0.1), vec2<f32>(1.6, 0.4), soft) * 1.5;
    let before = softbox(d, vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(-1.0, 0.0, 0.0), vec2<f32>(drift, 0.9), vec2<f32>(4.0, 0.6), soft * 1.6) * 0.5;
    let left = softbox(d, vec3<f32>(-0.72, 0.0, 0.69), vec3<f32>(0.69, 0.0, 0.72), vec2<f32>(drift * 0.3, 0.1), vec2<f32>(0.035, 1.5), 0.022) * 2.3;
    let right = softbox(d, vec3<f32>(0.78, 0.0, 0.62), vec3<f32>(0.62, 0.0, -0.78), vec2<f32>(drift * 0.3, 0.1), vec2<f32>(0.022, 1.5), 0.016) * 2.0;
    let rim = softbox(d, vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(1.0, 0.0, 0.0), vec2<f32>(0.0, -0.27), vec2<f32>(2.6, 0.13), soft) * frame.light.z * 1.3;
    return (over + before + left + right) * frame.light.y + rim + 0.02 * max(d.y, 0.0) + 0.012 * max(-d.y, 0.0);
}

// `Foil`'s ramp for a metal: deep to body below 1, body to its bright above, which keeps the metal's own hue
// however hard it is lit. 0 silver, 1 copper, 2 gold.
fn foil(metal: i32, t: f32) -> vec3<f32> {
    var deep = linear(vec3<f32>(51.0, 51.0, 61.0));
    var body = linear(vec3<f32>(199.0, 204.0, 219.0));
    var bright = vec3<f32>(1.5, 1.52, 1.6);
    if metal == 1 {
        deep = linear(vec3<f32>(80.0, 28.0, 14.0));
        body = linear(vec3<f32>(206.0, 104.0, 66.0));
        bright = linear(vec3<f32>(255.0, 150.0, 98.0)) * 1.6;
    } else if metal == 2 {
        deep = linear(vec3<f32>(92.0, 56.0, 13.0));
        body = linear(vec3<f32>(219.0, 166.0, 64.0));
        bright = linear(vec3<f32>(255.0, 205.0, 95.0)) * 1.6;
    }
    if t < 1.0 {
        return mix(deep * 0.25, body, smoothstep(0.0, 1.0, t));
    }
    // Past its bright, a streak of the lights themselves burns at its core.
    return mix(body, bright, 1.0 - exp(-(t - 1.0) * 1.3)) + vec3<f32>(1.0, 0.94, 0.8) * 0.5 * max(t - 3.0, 0.0);
}

// The holographic seal's spectrum, its seven inks turning with `phase`.
fn spectrum(phase: f32) -> vec3<f32> {
    let inks = array<vec3<f32>, 7>(
        vec3<f32>(218.0, 218.0, 107.0), vec3<f32>(144.0, 255.0, 144.0), vec3<f32>(107.0, 218.0, 218.0), vec3<f32>(144.0, 144.0, 255.0),
        vec3<f32>(218.0, 107.0, 218.0), vec3<f32>(255.0, 144.0, 144.0), vec3<f32>(240.0, 170.0, 120.0),
    );
    let at = fract(phase) * 7.0;
    let i = i32(floor(at));
    return linear(mix(inks[i], inks[(i + 1) % 7], smoothstep(0.0, 1.0, fract(at))));
}

// What a brushed foil mirrors along `r`: the studio smeared across the brushing, which runs level; `rough` 1 is
// gold's, less a clearer mirror.
fn brushed(r: vec3<f32>, n: vec3<f32>, grain: f32, rough: f32) -> f32 {
    var lit = 0.0;
    for (var i = -2; i <= 2; i++) {
        let lean = (f32(i) * 0.085 + grain) * rough;
        lit += studio(normalize(r + vec3<f32>(0.0, lean, 0.0) * (1.0 - abs(n.y)) + n * abs(n.y) * lean), 0.05 + 0.11 * rough);
    }
    return lit / 5.0;
}

// The light out of a box, by its foil: warm from gold, white from silver, ember from copper, every ink in turn
// from the holographic one.
fn beam_colour(metal: i32, along: f32) -> vec3<f32> {
    if metal == 0 {
        return vec3<f32>(0.92, 0.95, 1.0);
    }
    if metal == 1 {
        return vec3<f32>(1.0, 0.62, 0.38);
    }
    if metal == 3 {
        return mix(vec3<f32>(1.0), spectrum(along + frame.view.z * 0.06) * 2.6, 0.75);
    }
    return vec3<f32>(1.0, 0.74, 0.4);
}

// The light the glowing boxes throw on a point about them; a box's own walls stand under its mouth, unlit by it.
fn glow_on(p: vec3<f32>, n: vec3<f32>, own: i32) -> vec3<f32> {
    var lit = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let g = frame.gifts[i];
        if g.place.w <= 0.0 || g.tie.w <= 0.0 || i == own {
            continue;
        }
        let mouth = g.place.xyz + vec3<f32>(0.0, g.pose.y + 2.0 * g.shape.x * (1.0 - g.shape.y) * g.place.w, 0.0);
        let to = mouth - p;
        let far = length(to) / g.place.w;
        lit += beam_colour(i32(g.pose.z), 0.0) * g.tie.w * g.pose.w * max(dot(n, normalize(to)), 0.0) * 2.4 / (1.0 + far * far);
    }
    return lit;
}

// The holographic foil: a silver mirror embossed in facets, each a grating that splits the studio's light into
// the seal's inks by the angle it is seen at, so the colours sweep the faces as the gift or the camera turns.
fn hologram(q: vec3<f32>, local: vec3<f32>, r: vec3<f32>, silver: vec3<f32>, wide: f32, tight: f32) -> vec3<f32> {
    let a = abs(local);
    var on = q.xy;
    if a.x > a.z && a.x > a.y {
        on = q.zy;
    } else if a.y > a.x && a.y > a.z {
        on = q.xz;
    }
    // Facets a quarter turn askew, each with its own grating.
    let at = turned(on, 0.7854) * 3.4;
    let facet = floor(at);
    let odd = abs(((facet.x + facet.y) % 2.0 + 2.0) % 2.0 - 0.5) * 2.0 - 0.5;
    let grating = normalize(vec3<f32>(0.8, 0.45 + 0.5 * odd, 0.4));
    let ink = spectrum(dot(r, grating) * 1.9 + odd * 0.16 + (hash(facet) - 0.5) * 0.1);
    let soft = mix(vec3<f32>(dot(ink, vec3<f32>(0.3, 0.6, 0.1))), ink, 1.6);
    // The embossing's creases, and glints where a facet meets the light head on.
    let crease = smoothstep(0.0, 0.05, min(min(fract(at.x), 1.0 - fract(at.x)), min(fract(at.y), 1.0 - fract(at.y))));
    let spark = floor(at * 11.0);
    let glint = pow(hash(spark), 40.0) * pow(max(sin(dot(r, grating) * 31.0 + hash(spark + 1.0) * 6.28), 0.0), 6.0) * (0.3 + tight) * 9.0;
    return silver * (0.25 + 0.3 * crease) + max(soft, vec3<f32>(0.0)) * (0.07 + 1.5 * tight + 0.3 * wide) * (0.7 + 0.3 * crease) + spectrum(hash(spark + 2.0)) * glint * (1.0 - smoothstep(0.25, 0.5, length(fract(at * 11.0) - 0.5)));
}

// A gift's surface: its foil, its ribbon or its lit inside, seen from `v` at `p`.
fn gift_colour(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, what: f32, index: i32) -> vec3<f32> {
    let kind = what - 10.0 * f32(index);
    let g = frame.gifts[index];
    let metal = i32(g.pose.z);
    let q = in_gift(p, g);
    let air = open_air(index, p, n);
    let key = normalize(vec3<f32>(-0.45, 0.8, 0.4));
    let seen = shadow(index, p + n * 0.01, key);
    // A sheet of foil is never flat: each face swells a hair, so the studio slides across it as the gift turns.
    let swell = (q - vec3<f32>(0.0, g.shape.x, 0.0)) / vec3<f32>(1.0, g.shape.x, 1.0);
    let local = vec3<f32>(turned(n.xz, -g.pose.x).x, n.y, turned(n.xz, -g.pose.x).y);
    // Silver, the clearest mirror, bows the most: its lights curve across a face instead of filling it.
    var swells = array<f32, 4>(0.2, 0.07, 0.07, 0.1);
    let eased = local + swells[metal] * swell * (vec3<f32>(1.0) - abs(local));
    let bent = normalize(vec3<f32>(turned(eased.xz, g.pose.x).x, eased.y, turned(eased.xz, g.pose.x).y));
    let r = reflect(-v, bent);
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.0);
    // Its own light falls on its lid, never on the walls under its mouth.
    let near = glow_on(p, n, select(index, -1, kind == LIDFOIL));
    if kind == RIBBON {
        // Satin: the ink the seals are printed in, a long sheen across its weave; the holographic gift's is white.
        var silk = linear(vec3<f32>(20.0, 14.0, 5.0));
        if metal == 3 {
            silk = linear(vec3<f32>(236.0, 234.0, 228.0));
        }
        // Its threads run along the band and smear the studio across them, as satin does.
        let thread = noise(vec2<f32>((q.x + q.z) * 160.0, q.y * 6.0)) - 0.5;
        let across = normalize(cross(n, vec3<f32>(0.3, 1.0, 0.2)));
        var mirrored = 0.0;
        for (var i = -1; i <= 1; i++) {
            mirrored += studio(normalize(r + across * (f32(i) * 0.1 + thread * 0.025)), 0.14) / 3.0;
        }
        let sheen = pow(max(dot(normalize(key + v), n), 0.0), 26.0);
        let lit = ((0.25 + 0.75 * seen) * max(dot(n, key), 0.0) + 0.1) * frame.light.x;
        // A dark silk shines far above its own ink; a white one only a little.
        let shine = mix(silk * 3.0 + vec3<f32>(0.1, 0.095, 0.09), silk * 0.5, step(0.3, silk.g));
        return (silk * lit * mix(1.0, 0.7, step(0.3, silk.g)) + shine * (sheen * 1.2 * seen + mirrored * 1.6 + rim * 1.2 * frame.light.z) + silk * near) * air * g.pose.w;
    }
    // The brushing: fine level lines, which lean the mirror up and down by a hair; silver is the clearest mirror.
    var roughs = array<f32, 4>(0.4, 1.25, 1.0, 0.4);
    let rough = roughs[metal];
    let lines = noise(vec2<f32>(q.y * 240.0, (q.x + q.z) * 2.0)) - 0.5;
    let fine = noise(vec2<f32>(q.y * 900.0, (q.x - q.z) * 5.0)) - 0.5;
    let finest = noise(vec2<f32>(q.y * 3200.0, (q.x + q.z) * 9.0)) - 0.5;
    let mirrored = brushed(r, n, lines * 0.04 + fine * 0.035 + finest * 0.03, rough);
    let direct = max(dot(n, key), 0.0) * seen * frame.light.x;
    var t = 0.15 * frame.light.y + mirrored * 2.0 + direct * 0.07 + rim * 0.4 * frame.light.z;
    t *= 0.35 + 0.65 * air;
    var colour = foil(min(metal, 2), t);
    if metal == 3 {
        colour = hologram(q, local, r, foil(0, t), studio(r, 0.55), mirrored);
        if kind == INSIDE {
            // A fallen wall's inner face: the same foil, washed by the light that stands in its middle.
            colour = colour * 1.3 + beam_colour(metal, q.x * 0.2 + q.z * 0.13) * g.tie.w * 0.5 * exp(-length(q.xz) * 0.45);
        }
    }
    // A lifted box's underside sees only the floor.
    colour *= 1.0 - 0.94 * smoothstep(0.3, 0.8, -local.y) * step(q.y, 0.1);
    // The inside of an opened box, lit by the light it holds: brightest on its floor, a quarter of that at its rim.
    var within = step(max(abs(q.x), abs(q.z)), 1.0 - WALL + 0.012) * step(2.0 * WALL - 0.03, q.y) * step(q.y, 2.0 * g.shape.x + 0.02);
    if kind == LIDFOIL {
        within = 0.0;
    }
    let burning = mix(beam_colour(metal, q.x * 0.15), foil(min(metal, 2), 1.6), 0.6) * within * g.tie.w * (0.5 + 2.6 * pow(1.0 - clamp(q.y / (2.0 * g.shape.x), 0.0, 1.0), 2.0)) * (1.0 - 0.8 * g.shape.y);
    // Before the lid goes, the light it holds leaks out under its edge: a hairline, brightest at the seam.
    let seam = 2.0 * g.shape.x - 0.08;
    let leak = beam_colour(metal, q.x * 0.15) * g.shape.z * 3.2 * exp(-max(seam - q.y, 0.0) / 0.035) * step(q.y, seam + 0.01) * a_side(local) * (1.0 - within);
    return (colour + foil(min(metal, 2), 0.9) * near + burning + leak) * g.pose.w;
}

// 1 on a face that looks sideways, 0 on one that looks up or down.
fn a_side(local: vec3<f32>) -> f32 {
    return step(abs(local.y), 0.5);
}

// The floor: black gloss under a pool of light, the gifts' shadows on it; its mirror is laid by the picture.
fn floor_colour(p: vec3<f32>) -> vec3<f32> {
    let pool = exp(-dot(p.xz - frame.aim.xz, p.xz - frame.aim.xz) / (frame.aim.w * frame.aim.w));
    let up = vec3<f32>(0.0, 1.0, 0.0);
    return vec3<f32>(0.024, 0.023, 0.027) * pool * frame.light.x * floor_air(p) + glow_on(p, up, -1) * 0.3 * floor_air(p);
}
