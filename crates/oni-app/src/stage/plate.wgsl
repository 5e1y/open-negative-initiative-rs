// The film over the largest gift, laid flat in the camera's ray with no marching: the print's sheet, and before
// it the strip of 35 mm film, its perforations and edge print cut to the pixel. Lengths in frame half-heights.

// The strip's half-height, a frame's pitch along it, a perforation's pitch, and where the strip's end leads.
const STRIP: f32 = 1.458;
const PITCH: f32 = 3.167;
const LEAD: f32 = -8.0;
// The edge print's letters, five dots by five, and the line they spell along each frame.
const GLYPHS = array<u32, 15>(0u, 15255086u, 18667121u, 14815374u, 16267326u, 15269425u, 33061392u, 33061407u, 32641156u, 18157700u, 17318431u, 18732593u, 4591758u, 31504927u, 17727376u);
const MARKS = array<u32, 29>(1u, 2u, 3u, 0u, 4u, 5u, 6u, 7u, 8u, 9u, 0u, 6u, 3u, 10u, 11u, 0u, 0u, 0u, 14u, 12u, 13u, 0u, 0u, 0u, 0u, 14u, 12u, 13u, 5u);

// The light that the screen's curve brings back to `p`: a picture on the plate shows as its file holds it.
fn undeveloped(p: vec3<f32>) -> vec3<f32> {
    let y = clamp(p, vec3<f32>(0.0), vec3<f32>(0.995));
    return (sqrt((1.0 - y) * (1.0 - y) + 4.0 * y / 9.0) - (1.0 - y)) * 4.5 / 1.25;
}

fn pictured(picture: texture_2d<f32>, uv: vec2<f32>) -> vec3<f32> {
    return pow(textureSampleLevel(picture, smooth_sampler, clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999)), 0.0).rgb, vec3<f32>(2.2));
}

// One of the prints, by its place among them: they stand one over the other in one picture.
fn print_at(uv: vec2<f32>, which: f32) -> vec3<f32> {
    let row = (which + 3.0) % 3.0;
    return pictured(print, vec2<f32>(uv.x, (clamp(uv.y, 0.002, 0.998) + row) / 3.0));
}

// How much of a pixel `fp` wide a shape covers, from its signed distance.
fn covered(d: f32, fp: f32) -> f32 {
    return clamp(0.5 - d / fp, 0.0, 1.0);
}

fn box2(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// The edge print at `at`, in dots from the line's start and its foot: 1 on a dot; too small to read, its mean.
fn edge_print(at: vec2<f32>, fp: f32) -> f32 {
    if at.y < 0.0 || at.y >= 5.0 {
        return 0.0;
    }
    var marks = MARKS;
    var glyphs = GLYPHS;
    let cell = floor(at.x / 6.0);
    let column = i32(floor(at.x - cell * 6.0));
    let glyph = glyphs[marks[i32(cell) % 29]];
    if column > 4 {
        return 0.0;
    }
    let bit = f32((glyph >> u32(i32(floor(at.y)) * 5 + 4 - column)) & 1u);
    return mix(0.25 * step(0.5, f32(glyph)), bit, clamp(0.6 / fp, 0.0, 1.0));
}

// Where the camera's ray meets the plate, `FAR` where nothing of it stands.
fn plate_reach(start: vec3<f32>, along: vec3<f32>) -> f32 {
    if frame.plate.w <= 0.0 || frame.film.w + frame.turn.y <= 0.0 {
        return FAR;
    }
    let n = vec3<f32>(sin(frame.turn.x), 0.0, cos(frame.turn.x));
    let facing = dot(along, n);
    if abs(facing) < 0.0001 {
        return FAR;
    }
    let t = dot(frame.plate.xyz - start, n) / facing;
    let d = (start + along * t - frame.plate.xyz) / frame.plate.w;
    let across = abs(dot(d, vec3<f32>(n.z, 0.0, -n.x)));
    if t <= 0.0 || abs(d.y) > 1.7 || (frame.film.w <= 0.0 && across > 1.7 + 4.4 * frame.fan.x) {
        return FAR;
    }
    return t;
}

// One print's sheet, the `k`th of the fan (0 the middle), laid over `colour`: white paper with a border, the photo
// until the film is developed, then its print on a stock, the photo laid back over it up to the edge a hand drags.
fn sheet_over(colour: vec3<f32>, at: vec2<f32>, fp: f32, k: f32, gloss: f32) -> vec3<f32> {
    let size = 1.0 - 0.14 * abs(k) * frame.fan.x;
    let local = (at - vec2<f32>(k * 3.1 * frame.fan.x, 0.0)) / size;
    // The photo, before it is exposed, sits in the film's own gate: no border about it yet.
    let border = 0.065 * smoothstep(0.4, 0.6, frame.strip.y);
    let paper = covered(box2(local, vec2<f32>(1.5 + border, 1.0 + border), 0.01) * size, fp) * frame.turn.y;
    if paper <= 0.0 {
        return colour;
    }
    let uv = vec2<f32>(local.x / 3.0 + 0.5, 0.5 - local.y / 2.0);
    let stock = floor(frame.show.x);
    var picture = pictured(photo, uv);
    if frame.strip.y > 0.5 {
        picture = mix(print_at(uv, stock + k), print_at(uv, stock + k + 1.0), smoothstep(0.0, 1.0, fract(frame.show.x)));
    }
    var line = 0.0;
    if frame.lens.y > 0.0 && k == 0.0 {
        picture = mix(picture, pictured(photo, uv), covered((uv.x - frame.lens.y) * 3.0 * size, fp));
        line = covered(abs(uv.x - frame.lens.y) * 3.0 * size - 0.004, fp);
    }
    let light = frame.turn.z * (1.0 - 0.12 * abs(k));
    let inside = covered(box2(local, vec2<f32>(1.5, 1.0), 0.0) * size, fp);
    let sheet = mix(undeveloped(vec3<f32>(0.955, 0.95, 0.93) * min(light, 1.0)), undeveloped(picture * min(light, 1.0)) * max(light, 1.0), inside);
    return mix(colour, sheet + vec3<f32>(gloss * 0.03) + vec3<f32>(1.0, 0.97, 0.9) * line * inside * 2.2, paper);
}

// The strip's frame at `uv`: undeveloped emulsion holding a latent picture, then the negative as its densities
// come up, its grain and halation with them; lit from behind where the scanner has passed, inverted past its edge.
fn frame_colour(uv: vec2<f32>, fp: f32, ground: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
    let scene = pictured(photo, uv);
    let seen = dot(scene, vec3<f32>(0.3, 0.6, 0.1));
    let latent = ground + vec3<f32>(1.0, 0.95, 0.9) * seen * 0.2 * frame.strip.z;
    var film = pictured(negative, uv);
    // A negative holds a deep picture under its mask: its densities drawn out so it reads on a screen.
    film = base * pow(film / base, vec3<f32>(1.9));
    // Halation: the scene's lights spread in the red-sensitive layer, which the negative holds as more cyan.
    var about = 0.0;
    for (var k = 0; k < 6; k++) {
        let turn = f32(k) * 1.0472;
        let far = pictured(photo, uv + vec2<f32>(cos(turn), sin(turn) * 1.5) * 0.011);
        about += smoothstep(0.45, 0.9, dot(far, vec3<f32>(0.3, 0.6, 0.1))) / 6.0;
    }
    film *= vec3<f32>(1.0 - 0.55 * about * frame.strip.w, 1.0 - 0.12 * about * frame.strip.w, 1.0);
    // The grain: each layer its own specks, soft-edged, shimmering while they settle, seen once one fills a pixel.
    let at = uv * vec2<f32>(2100.0, 1400.0);
    let age = floor(frame.view.z * 14.0) * frame.show.y * 7.3;
    let specks = vec3<f32>(noise(at + age), noise(at * 1.07 + 31.0 + age), noise(at * 0.93 + 67.0 + age)) - 0.5;
    film *= vec3<f32>(1.0) + specks * 1.5 * min(frame.turn.w, 1.0) * clamp(0.0016 / fp - 0.2, 0.0, 1.0);
    // The picture comes up from its densest parts first, as in the developer.
    let risen = smoothstep(0.0, 1.0, clamp(frame.strip.y * 1.6 - 0.6 * seen, 0.0, 1.0));
    return mix(latent, film, risen);
}

// The plate at `p`, `t` along the camera's ray, laid over what stands `behind` it: the strip, then the sheets.
fn plate_over(p: vec3<f32>, along: vec3<f32>, t: f32, behind: vec3<f32>) -> vec3<f32> {
    let half = frame.plate.w;
    let n = vec3<f32>(sin(frame.turn.x), 0.0, cos(frame.turn.x));
    let right = vec3<f32>(n.z, 0.0, -n.x);
    let d = (p - frame.plate.xyz) / half;
    let at = vec2<f32>(dot(d, right), d.y);
    let fp = t / (frame.eye.w * frame.view.y * half);
    // A sheet of film or paper is never flat: its curl slides the studio's lights across it.
    let xf = at.x - frame.strip.x;
    let gloss = studio(reflect(along, normalize(n - vec3<f32>(0.0, 0.3 * at.y, 0.0))), 0.2);
    var colour = behind;
    let strip = covered(max(abs(at.y) - STRIP, LEAD - xf), fp) * frame.film.w;
    if strip > 0.0 {
        colour = strip_over(colour, vec2<f32>(xf, at.y), fp, gloss, strip);
    }
    if frame.turn.y > 0.0 {
        if frame.fan.x > 0.001 {
            colour = sheet_over(sheet_over(colour, at, fp, -1.0, gloss), at, fp, 1.0, gloss);
        }
        colour = sheet_over(colour, at, fp, 0.0, gloss);
    }
    return colour;
}

// The strip at `at`, along it from its frame's middle and across it, covering `strip` of the pixel, over `behind`.
fn strip_over(behind: vec3<f32>, at: vec2<f32>, fp: f32, gloss: f32, cover: f32) -> vec3<f32> {
    let pitch = PITCH / 8.0;
    let hole = box2(vec2<f32>((fract(at.x / pitch) - 0.5) * pitch, abs(at.y) - 1.208), vec2<f32>(0.1165, 0.0825), 0.03);
    // The scanner's lamp stands behind the film: a hot bar, and its light bare through the perforations it has passed.
    let u = (at.x + 1.5) / 3.0;
    let passed = frame.film.z * smoothstep(frame.film.x + 0.004, frame.film.x - 0.004, u);
    let bar = (exp(-pow((u - frame.film.x) / 0.0035, 2.0)) * 9.0 + exp(-pow((u - frame.film.x) / 0.02, 2.0)) * 1.1) * step(-1.0, frame.film.x) * step(frame.film.x, 1.95);
    let lamp = vec3<f32>(1.0, 0.97, 0.9);
    let through = behind + lamp * (bar * 0.6 + passed * 1.6);
    let developed = frame.strip.y;
    let base = linear(vec3<f32>(214.0, 120.0, 58.0));
    let ground = mix(vec3<f32>(0.1, 0.082, 0.086), base, developed);
    let inside = covered(box2(at, vec2<f32>(1.5, 1.0), 0.02), fp);
    let uv = vec2<f32>(u, 0.5 - at.y / 2.0);
    var film = mix(ground, frame_colour(uv, fp, ground, base), inside);
    // The edge print, exposed at the factory and come up with the picture; a row of bars along the other edge.
    let dot_size = 0.018;
    let print = edge_print(vec2<f32>((at.x - LEAD) / dot_size, (at.y - 1.33) / dot_size), fp / dot_size);
    let bars = step(0.55, hash(vec2<f32>(floor(at.x * 16.0), 9.0))) * step(abs(at.y + 1.375), 0.04) * step(fract(at.x / PITCH + 0.1), 0.32);
    film *= 1.0 - 0.72 * max(print, bars) * developed;
    // Where the scanner has passed, the film is read through its mask: lit from behind, its orange taken out.
    film = mix(film * mix(0.6, 1.0, frame.fan.z), film / base * 0.62, passed) * (1.0 + 1.2 * frame.fan.z);
    // The inversion: every tone crosses to its opposite at once, through grey.
    film = mix(film, print_at(uv, 0.0), frame.film.y * inside);
    let own = min(frame.turn.z, 1.0);
    let seeps = behind * vec3<f32>(1.0, 0.5, 0.2) * 0.16 * developed * (1.0 - 0.7 * inside);
    let sheet = undeveloped(film * own) * max(frame.turn.z, 1.0) + seeps + vec3<f32>(gloss * 0.04) + lamp * bar * (0.25 + 0.75 * inside);
    return mix(behind, mix(through, sheet, 1.0 - covered(hole, fp)), cover);
}
