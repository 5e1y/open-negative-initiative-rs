//! `digital_stages <photo> <stock> <out>`: a digital photo shot on `stock` at its rests, written stage by stage as the
//! app's chain makes it, for the process board: each stage's pixels as f32 (`<n>.f32`: width, height, then RGB rows) and
//! `stages.json` saying what each holds, its axis and its points; the sensor's counts as `01-mosaic.u16`.

use oni_app::render::{self, Device, Engine, Reading};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Inputs, Request, conversion, film, grain, halation, layers, stages};
use oni_render::{Projection, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

struct Board {
    dir: PathBuf,
    stages: Vec<Value>,
}

impl Board {
    /// One stage's pixels and what they are: `space` names the axis its histogram is read on.
    fn stage(&mut self, name: &str, (w, h): (u32, u32), pixels: &[[f32; 4]], meta: Value) {
        let file = format!("{:02}-{name}.f32", self.stages.len() + 1);
        let mut f = std::io::BufWriter::new(std::fs::File::create(self.dir.join(&file)).unwrap());
        f.write_all(&w.to_le_bytes()).unwrap();
        f.write_all(&h.to_le_bytes()).unwrap();
        for p in pixels {
            for v in &p[..3] {
                f.write_all(&v.to_le_bytes()).unwrap();
            }
        }
        let mut meta = meta;
        meta["file"] = json!(file);
        meta["name"] = json!(name);
        println!("{file}: {w} x {h}, channel means {:?}", means(pixels));
        self.stages.push(meta);
    }
}

fn means(pixels: &[[f32; 4]]) -> [f32; 3] {
    let n = pixels.len().max(1) as f64;
    std::array::from_fn(|c| (pixels.iter().map(|p| f64::from(p[c])).sum::<f64>() / n) as f32)
}

fn read(g: &mut Graph, image: &Image) -> Vec<[f32; 4]> {
    g.render(image, image.extent(), None).unwrap()
}

fn size(image: &Image) -> (u32, u32) {
    let e = image.extent();
    (e.w, e.h)
}

/// The widest gap between two renders of a stage, each channel's.
fn apart(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f32::max)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (photo, out) = (Path::new(&args[0]), PathBuf::from(&args[2]));
    let stock = VirtualStock::of(Box::leak(args[1].clone().into_boxed_str()));
    std::fs::create_dir_all(&out).unwrap();
    let mut b = Board { dir: out.clone(), stages: Vec::new() };
    let s = PipelineSettings::rest(ConversionMode::Digital, true).picking(stock);
    let film_ = s.film().expect("a digital photo on its film");
    let st = film::stock(stock);

    // The sensor's counts, before any demosaic.
    let m = oni_decode::mosaic(photo).expect("a mosaic");
    let mut f = std::fs::File::create(out.join("01-mosaic.u16")).unwrap();
    let bytes: Vec<u8> = m.counts.iter().flat_map(|v| v.to_le_bytes()).collect();
    f.write_all(&bytes).unwrap();
    let pattern: Vec<usize> = (0..4).map(|i| m.colour(i % 2, i / 2)).collect();
    b.stages.push(json!({"file": "01-mosaic.u16", "name": "mosaic", "width": m.width, "height": m.height, "pattern": pattern, "black": m.black, "white": m.white}));

    // The demosaic on the 5000 K multipliers, in the camera's own channels, then through its matrix to the working space.
    let mut own = s.clone();
    own.camera_channels = true;
    let cam = render::decode(photo, Reading::of(&own)).expect("a decode");
    b.stage("camera", (cam.linear.width as u32, cam.linear.height as u32), &cam.linear.pixels, json!({"space": "linear"}));
    let decoded = render::decode(photo, Reading::of(&s)).expect("a decode");
    b.stage("working", (decoded.linear.width as u32, decoded.linear.height as u32), &decoded.linear.pixels, json!({"space": "linear"}));

    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(decoded).unwrap();
    let (g, p) = e.parts();
    let preview = Image::new(&negative.preview);
    let scan = preview.extent();
    let scale = f64::from(scan.w) / negative.full_width;
    b.stage("preview", size(&preview), &read(g, &preview), json!({"space": "linear", "scale": scale}));

    let inputs = Inputs { patches: Default::default(), custom_flat_field: None, seed: negative.seed, plate: Some(&negative.preview) };
    let healed = p.healed(g, preview.clone(), &s, scale, &inputs).unwrap();
    let healed_px = read(g, &healed);
    b.stage("healed", size(&healed), &healed_px, json!({"space": "linear"}));
    let white = conversion::scene_white(&s).expect("a film with its light");
    let lit = healed.clone().then(stages::color_matrix(&white));
    let lit_px = read(g, &lit);
    b.stage("lit", size(&lit), &lit_px, json!({"space": "linear", "matrix": [white.r_vector, white.g_vector, white.b_vector]}));

    // The scene as the film sees it: XYZ adapted by CAT16 to the film's white, inside the exposure kernel.
    let k = st.kernel_at(film_.exposure);
    let xyz: Vec<[f32; 4]> = lit_px.iter().map(|q| {
        let v: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| k.to_film[r][c] * q[c]).sum());
        [v[0], v[1], v[2], 1.0]
    }).collect();
    b.stage("film-xyz", size(&lit), &xyz, json!({"space": "linear", "to_film": k.to_film}));
    let (tw, th) = k.table_size();
    let table = g.upload(tw as u32, th as u32, &st.table).unwrap();
    let tables = [Image::new(&table)];
    let exposed = lit.clone().then_reading(stages::virtual_exposure(&k), &tables);
    let exposed_px = read(g, &exposed);
    let cpu_exposed: Vec<[f32; 4]> = lit_px.iter().map(|q| cpu::virtual_exposure(*q, &k, &st.table)).collect();
    println!("exposure: GPU against its CPU reference {:.2e}", apart(&exposed_px, &cpu_exposed));
    b.stage("exposed", size(&exposed), &exposed_px, json!({"space": "log-exposure", "layers": true, "shooting_stops": film_.exposure}));

    // The emulsion's blur, then the halation drawn on the film's plate.
    let sigma = layers::bloom(&s, scan);
    let bloomed = if sigma > oni_settings::axis::pipeline::BLUR_FLOOR { stages::blurred(exposed.clone(), sigma, false) } else { exposed.clone() };
    b.stage("bloomed", size(&bloomed), &read(g, &bloomed), json!({"space": "log-exposure", "layers": true, "sigma_px": sigma}));
    let plate_lit = Image::new(&negative.preview);
    let plate_lit = p.healed(g, plate_lit, &s, scale, &inputs).unwrap().then(stages::color_matrix(&white));
    let plate = plate_lit.then_reading(stages::virtual_exposure(&k), &tables);
    let glow = p.glow(g, &s, Some(scan), plate, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]).unwrap();
    let halated = match &glow {
        Some(glow) => {
            let plate = Image::new(&glow.plate);
            b.stage("glow", size(&plate), &read(g, &plate), json!({"space": "log-exposure", "layers": true, "amount": s.halation.amount}));
            halation::halated_from(bloomed.clone(), &s, glow)
        }
        None => bloomed.clone(),
    };
    b.stage("halated", size(&halated), &read(g, &halated), json!({"space": "log-exposure", "layers": true}));

    // The development on the sheet's curves: each layer's light through its own dye.
    let developed = halated.then_reading(stages::virtual_development(&k), &tables);
    b.stage("developed", size(&developed), &read(g, &developed), json!({"space": "density", "layers": true}));

    // The scan: the dyes and the orange base under the D50 lamp, seen by the D850, then its matrix to the working space.
    let sc = st.scan.as_ref().expect("a colour negative is scanned");
    let scan_table = g.upload(sc.projection.bands as u32, 2, &sc.table).unwrap();
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let sensor = developed.clone().then_reading(stages::projection(&Projection { to_working: identity, ..sc.projection }), &[Image::new(&scan_table)]);
    b.stage("scanner-camera", size(&sensor), &read(g, &sensor), json!({"space": "linear", "lamp": film::scanner::LAMP_NAME, "sensor": film::scanner::SENSOR_NAME}));
    let scanned = developed.then_reading(stages::projection(&sc.projection), &[Image::new(&scan_table)]);
    b.stage("scanned", size(&scanned), &read(g, &scanned), json!({"space": "linear", "to_working": sc.projection.to_working}));
    let grained = match grain::grain(&s, scan, scale).map(|k| k.seeded(negative.seed)) {
        Some(k) => grain::grained_over(scanned, &healed, &k),
        None => scanned,
    };
    let grained_px = read(g, &grained);
    b.stage("grained", size(&grained), &grained_px, json!({"space": "linear", "amount": s.grain.amount, "size": s.grain.size}));

    // The frozen scan as the canvas keeps it, half floats, against the stages above.
    let request = Request { full_width: Some(negative.full_width), ..Request::default() };
    let frozen = p.scan(g, &negative.preview, &s, &request, &inputs).unwrap();
    let frozen: Source = g.materialise_half(&frozen).unwrap();
    let frozen_img = Image::new(&frozen);
    let frozen_px = read(g, &frozen_img);
    let gap = apart(&frozen_px, &grained_px);
    println!("frozen scan against the stages: {gap:.2e}");
    b.stage("frozen", size(&frozen_img), &frozen_px, json!({"space": "linear", "against_stages": gap}));

    // The negative read on its two points, the fused stage 4-7 laid out on the CPU: Lighting, gains, density, shares, inversion.
    let k47 = conversion::stage47(&s);
    let lit_neg: Vec<[f32; 4]> = frozen_px.iter().map(|q| k47.negative.as_ref().map_or(*q, |n| cpu::negative(*q, n))).collect();
    b.stage("lighting", size(&frozen_img), &lit_neg, json!({"space": "linear", "at_rest": k47.negative.is_none()}));
    let rest = k47.after_negative();
    let gained: Vec<[f32; 4]> = lit_neg.iter().map(|q| {
        let v: [f32; 3] = std::array::from_fn(|c| (q[c] + rest.pedestal) * rest.gain[c]);
        [v[0], v[1], v[2], 1.0]
    }).collect();
    b.stage("gains", size(&frozen_img), &gained, json!({"space": "linear", "stops": s.gains.stops, "pedestal": rest.pedestal}));
    let density: Vec<[f32; 4]> = gained.iter().map(|q| {
        let v: [f32; 3] = std::array::from_fn(|c| -q[c].max(rest.log_guard).log10());
        [v[0], v[1], v[2], 1.0]
    }).collect();
    let points = [rest.red, rest.green, rest.blue].map(|l| [l[0], l[1]]);
    b.stage("density", size(&frozen_img), &density, json!({"space": "density-values", "points": points}));
    let shares: Vec<[f32; 4]> = lit_neg.iter().map(|q| {
        let v = cpu::windowed(*q, &k47);
        [v[0], v[1], v[2], 1.0]
    }).collect();
    b.stage("shares", size(&frozen_img), &shares, json!({"space": "share"}));
    let inverted: Vec<[f32; 4]> = frozen_px.iter().map(|q| cpu::stage47(*q, &k47)).collect();
    let by_hand: Vec<[f32; 4]> = shares.iter().map(|q| cpu::inversion(*q)).collect();
    println!("inversion laid by hand against the fused stage: {:.2e}", apart(&inverted, &by_hand));
    let shown = match k47.shown { oni_render::Shown::Inversion => "inversion", oni_render::Shown::Paper(_) => "paper", oni_render::Shown::Curve => "curve" };
    b.stage("inverted", size(&frozen_img), &inverted, json!({"space": "linear", "shown": shown, "paper": s.print_paper.map(|p| p.key())}));

    // The finishing's chroma denoise alone, the sharpening it shares one pass with held at zero, then the whole chain.
    let soft = PipelineSettings { sharpen: oni_settings::blocks::Sharpen { amount: 0.0, ..s.sharpen }, ..s.clone() };
    let denoised = p.on_scan(g, &frozen, &soft, &request).unwrap();
    let denoised_px = read(g, &denoised);
    b.stage("denoised", size(&denoised), &denoised_px, json!({"space": "linear", "chroma": [s.chroma.radius, s.chroma.force], "against_inversion": apart(&denoised_px, &inverted)}));
    // The rest of the chain on the GPU, as the canvas renders it: the finishing and the output clip.
    let full = p.on_scan(g, &frozen, &s, &request).unwrap();
    let full_px = read(g, &full);
    let finishing = apart(&full_px, &inverted);
    println!("the app's render against the inversion: {finishing:.2e}");
    b.stage("rendered", size(&full), &full_px, json!({"space": "linear", "against_inversion": finishing, "sharpen": [s.sharpen.radius, s.sharpen.amount, s.sharpen.halo]}));

    // The bare film and a neutral at the sensor's saturation through the same stages, each channel's D-min and D-max.
    let sat = film::saturation();
    let ends = g.upload(2, 1, &[[0.0, 0.0, 0.0, 1.0], [sat, sat, sat, 1.0]]).unwrap();
    let ends_exposed = Image::new(&ends).then_reading(stages::virtual_exposure(&k), &tables);
    let ends_developed = ends_exposed.then_reading(stages::virtual_development(&k), &tables);
    let ends_camera = ends_developed.clone().then_reading(stages::projection(&Projection { to_working: identity, ..sc.projection }), &[Image::new(&scan_table)]);
    let ends_scanned = ends_developed.clone().then_reading(stages::projection(&sc.projection), &[Image::new(&scan_table)]);
    let rgb = |v: Vec<[f32; 4]>| v.iter().map(|p| [p[0], p[1], p[2]]).collect::<Vec<_>>();
    let (open_camera, open_working) = film::scanner::open_lamp();
    let negative_ends = json!({
        "developed": {"open": [1.0, 1.0, 1.0], "ends": rgb(read(g, &ends_developed))},
        "scanner-camera": {"open": open_camera, "ends": rgb(read(g, &ends_camera))},
        "working": {"open": open_working, "ends": rgb(read(g, &ends_scanned))},
        "gains": s.gains.stops,
    });
    println!("bare film and saturation: {negative_ends}");

    let manifest = json!({"negative": negative_ends, "photo": photo.file_name().map(|n| n.to_string_lossy().into_owned()), "stock": stock.name(), "stages": b.stages});
    std::fs::write(out.join("stages.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}
