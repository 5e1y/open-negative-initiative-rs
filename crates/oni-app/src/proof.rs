//! `Proof.applied` on the GPU: every picture the editor shows drawn as the export will write it,
//! laid after the pipeline and before the display kernel. Never optional.

use oni_colour::{PaperCorrection, Proof, Space, pcs};
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_settings::PipelineSettings;
use std::sync::{Arc, Mutex};

/// The proof the interface asks for, read by the render thread at each render.
pub type Wanted = Arc<Mutex<Proof>>;

pub fn wanted(space: Space, colour_only: bool) -> Wanted {
    Arc::new(Mutex::new(Proof { colour_only, ..Proof::new(space) }))
}

/// The engine's half: the proof to lay and the paper's cube once laid on the graph.
#[derive(Default)]
pub struct Proofing {
    pub wanted: Option<Wanted>,
    /// A profile's cube, keyed by `Proof::cube`'s own key.
    cube: Option<(String, Source)>,
}

impl Proofing {
    /// The proof as the photo's settings correct it, or the picture as it is when the profile
    /// cannot be read, as `Proof.applied` keeps it rather than a blank.
    pub fn laid(&mut self, graph: &mut Graph, image: Image, settings: &PipelineSettings) -> Result<Image, String> {
        let Some(proof) = self.wanted.as_ref().and_then(|w| w.lock().ok().map(|p| p.clone())) else { return Ok(image) };
        let proof = proof.correcting(settings.paper_correction);
        Ok(match self.stage(graph, &proof)? {
            Some(Laid::Matrix(stage)) => image.then(stage),
            Some(Laid::Cube(stage, cube)) => image.then_reading(stage, &[Image::new(&cube)]),
            None => image,
        })
    }

    fn stage(&mut self, graph: &mut Graph, proof: &Proof) -> Result<Option<Laid>, String> {
        if let Space::Profile(profile) = &proof.space {
            let Some(cube) = Proof::cube(profile, proof.colour_only) else { return Ok(None) };
            let key = format!("{}|{}", profile.path, proof.colour_only);
            let held = match &self.cube {
                Some((k, source)) if *k == key => source.clone(),
                _ => {
                    let n = cube.side as u32;
                    let nodes: Vec<[f32; 4]> = cube.data.chunks_exact(4).map(|p| [p[0], p[1], p[2], 1.0]).collect();
                    let source = graph.upload(n * n, n, &nodes).map_err(|e| e.to_string())?;
                    self.cube = Some((key, source.clone()));
                    source
                }
            };
            let correction = if proof.space.paper().is_some() { proof.correction } else { PaperCorrection::default() };
            return Ok(Some(Laid::Cube(CubeProof::new(cube.side, correction), held)));
        }
        Ok(MatrixProof::for_space(&proof.space).map(Laid::Matrix))
    }
}

enum Laid {
    Matrix(MatrixProof),
    Cube(CubeProof, Source),
}

/// A matrix space: the working light into the space's own, clipped there, then back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatrixProof {
    pub to: [[f32; 3]; 3],
    pub back: [[f32; 3]; 3],
}

impl MatrixProof {
    pub fn for_space(space: &Space) -> Option<MatrixProof> {
        let m = space.from_working()?;
        let back = pcs::inverse(&m)?;
        let f = |m: pcs::Mat3| m.map(|r| r.map(|v| v as f32));
        Some(MatrixProof { to: f(m), back: f(back) })
    }

    /// The kernel's CPU reference, which `Proof::proofed` also is.
    pub fn apply(&self, p: [f32; 4]) -> [f32; 4] {
        let mul = |m: &[[f32; 3]; 3], v: [f32; 3]| std::array::from_fn::<f32, 3, _>(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2]);
        let clipped = mul(&self.to, [p[0], p[1], p[2]]).map(|v| v.clamp(0.0, 1.0));
        let out = mul(&self.back, clipped);
        [out[0], out[1], out[2], p[3]]
    }
}

const MATRIX_SHADER: &str = r#"
struct Params { to0: vec4<f32>, to1: vec4<f32>, to2: vec4<f32>, back0: vec4<f32>, back1: vec4<f32>, back2: vec4<f32> }
@group(0) @binding(3) var<uniform> P: Params;

fn pixel(p: vec2<i32>) -> vec4<f32> {
    let s = load(p);
    let to = clamp(vec3<f32>(dot(P.to0.xyz, s.rgb), dot(P.to1.xyz, s.rgb), dot(P.to2.xyz, s.rgb)), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(dot(P.back0.xyz, to), dot(P.back1.xyz, to), dot(P.back2.xyz, to), s.a);
}
"#;

fn rows(m: &[[f32; 3]; 3]) -> [[f32; 4]; 3] {
    m.map(|r| [r[0], r[1], r[2], 0.0])
}

impl Stage for MatrixProof {
    fn name(&self) -> &'static str {
        "proof-matrix"
    }

    fn shader(&self) -> &'static str {
        MATRIX_SHADER
    }

    fn params(&self) -> Vec<u8> {
        let all: Vec<[f32; 4]> = rows(&self.to).into_iter().chain(rows(&self.back)).collect();
        bytemuck::cast_slice(&all).to_vec()
    }
}

/// A profile: the paper's gains on a paper, then its cube on gamma-encoded Rec. 2020, read
/// trilinearly with red fastest, as `CIColorCubeWithColorSpace` reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeProof {
    pub side: usize,
    pub gain: [f32; 3],
}

impl CubeProof {
    pub fn new(side: usize, correction: PaperCorrection) -> CubeProof {
        let gain = if correction.is_neutral() { [1.0; 3] } else { correction.stops.map(f32::exp2) };
        CubeProof { side, gain }
    }
}

const CUBE_SHADER: &str = r#"
struct Params { head: vec4<u32>, gain: vec4<f32> }
@group(0) @binding(3) var<uniform> P: Params;

const A: f32 = 1.09929682680944;
const BETA: f32 = 0.018053968510807;

fn encode(l: f32) -> f32 {
    let a = abs(l);
    let v = select(A * pow(a, 0.45) - (A - 1.0), 4.5 * a, a < BETA);
    return sign(l) * v;
}

fn decode(e: f32) -> f32 {
    let v = abs(e);
    let l = select(pow((v + (A - 1.0)) / A, 1.0 / 0.45), v / 4.5, v < 4.5 * BETA);
    return sign(e) * l;
}

fn node(r: u32, g: u32, b: u32, n: u32) -> vec3<f32> { return load1(vec2<i32>(i32(g * n + r), i32(b))).rgb; }

fn pixel(p: vec2<i32>) -> vec4<f32> {
    let s = load(p);
    let n = P.head.w;
    let c = s.rgb * P.gain.rgb;
    let f = clamp(vec3<f32>(encode(c.r), encode(c.g), encode(c.b)), vec3<f32>(0.0), vec3<f32>(1.0)) * f32(n - 1u);
    let lo = min(vec3<u32>(floor(f)), vec3<u32>(n - 2u));
    let t = f - vec3<f32>(lo);
    let hi = lo + 1u;
    let c00 = mix(node(lo.x, lo.y, lo.z, n), node(hi.x, lo.y, lo.z, n), t.x);
    let c10 = mix(node(lo.x, hi.y, lo.z, n), node(hi.x, hi.y, lo.z, n), t.x);
    let c01 = mix(node(lo.x, lo.y, hi.z, n), node(hi.x, lo.y, hi.z, n), t.x);
    let c11 = mix(node(lo.x, hi.y, hi.z, n), node(hi.x, hi.y, hi.z, n), t.x);
    let o = mix(mix(c00, c10, t.y), mix(c01, c11, t.y), t.z);
    return vec4<f32>(decode(o.r), decode(o.g), decode(o.b), s.a);
}
"#;

impl Stage for CubeProof {
    fn name(&self) -> &'static str {
        "proof-cube"
    }

    fn shader(&self) -> &'static str {
        CUBE_SHADER
    }

    fn params(&self) -> Vec<u8> {
        let head = [0u32, 0, 0, self.side as u32];
        let mut out = bytemuck::bytes_of(&head).to_vec();
        out.extend_from_slice(bytemuck::bytes_of(&[self.gain[0], self.gain[1], self.gain[2], 0.0f32]));
        out
    }

    fn aux_region(&self, _: usize, _: Rect) -> Rect {
        let n = self.side as u32;
        Rect::new(0, 0, n * n, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Device;

    /// Light past every matrix space's gamut, and a grey, on a 16 × 4 frame.
    fn colours() -> Vec<[f32; 4]> {
        (0..64)
            .map(|i| {
                let t = (i % 16) as f32 / 15.0;
                match i / 16 {
                    0 => [t * 1.4, 0.02, 0.01, 1.0],
                    1 => [0.01, t, 0.9 - t * 0.5, 1.0],
                    2 => [t * 0.18, t * 0.18, t * 0.18, 1.0],
                    _ => [-0.05 + t, 1.2 * t, 0.3, 1.0],
                }
            })
            .collect()
    }

    #[test]
    fn the_matrix_proof_on_the_gpu_is_the_crate_s_proof() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the proof is not exercised");
            return;
        };
        let mut graph = Graph::from_device(device.device.clone(), device.queue.clone(), "proof".into(), oni_graph::TEXTURE_SIDE);
        let pixels = colours();
        let source = graph.upload(16, 4, &pixels).unwrap();
        let mut proofing = Proofing { wanted: Some(wanted(Space::Srgb, false)), ..Proofing::default() };
        let image = proofing.laid(&mut graph, Image::new(&source), &PipelineSettings::default()).unwrap();
        let got = graph.render(&image, Rect::new(0, 0, 16, 4), None).unwrap();
        let want = Proof::new(Space::Srgb).proofed(pixels.as_flattened()).unwrap();
        let worst = got.as_flattened().iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(worst < 1e-5, "{worst} from oni-colour's proof");
        // Twin: the proof clips, so the reddest light comes back changed; Rec. 2020 itself would not.
        let moved = got.iter().zip(&pixels).map(|(a, b)| (a[0] - b[0]).abs()).fold(0.0, f32::max);
        assert!(moved > 0.05, "the sRGB proof left light outside sRGB untouched");
        let wide = Proof::new(Space::DisplayP3).proofed(pixels.as_flattened()).unwrap();
        assert!(got.as_flattened().iter().zip(&wide).any(|(a, b)| (a - b).abs() > 1e-3), "sRGB and P3 proof alike");
    }

    #[test]
    fn the_cube_kernel_reads_the_cube_as_the_crate_reads_it() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the cube proof is not exercised");
            return;
        };
        let mut graph = Graph::from_device(device.device.clone(), device.queue.clone(), "proof".into(), oni_graph::TEXTURE_SIDE);
        // A cube that is not the identity: each node swaps red and blue and darkens green.
        let n = 5usize;
        let mut data = Vec::new();
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let s = 1.0 / (n - 1) as f32;
                    data.extend([b as f32 * s, g as f32 * s * 0.7, r as f32 * s, 1.0]);
                }
            }
        }
        let cube = oni_colour::Cube { side: n, data };
        let nodes: Vec<[f32; 4]> = cube.data.chunks_exact(4).map(|p| [p[0], p[1], p[2], 1.0]).collect();
        let table = graph.upload((n * n) as u32, n as u32, &nodes).unwrap();
        let pixels = colours();
        let source = graph.upload(16, 4, &pixels).unwrap();
        let correction = PaperCorrection { stops: [0.25, 0.0, -0.4] };
        let stage = CubeProof::new(n, correction);
        let image = Image::new(&source).then_reading(stage, &[Image::new(&table)]);
        let got = graph.render(&image, Rect::new(0, 0, 16, 4), None).unwrap();
        let mut want = pixels.as_flattened().to_vec();
        correction.apply(&mut want);
        cube.apply(&mut want);
        let worst = got.as_flattened().iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(worst < 1e-4, "{worst} from the cube's CPU reference");
        // Twin: without the paper's gains the kernel lands elsewhere.
        let mut bare = pixels.as_flattened().to_vec();
        cube.apply(&mut bare);
        assert!(got.as_flattened().iter().zip(&bare).any(|(a, b)| (a - b).abs() > 1e-2));
    }
}
