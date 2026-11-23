//! `dye_spill <out> <side> <spektrafilm profile | -> <name=m00,..,m22>... -- <raw>...`: each scan on ONI's own print at
//! rest on its sidecar's windows, the CPU reference alone: with no separation (`off`), with the stock's data-sheet dyes
//! (`now`), with a former spektrafilm profile's dyes (`old`), and with each named 3x3 laid on the windows' shares.
//! Also `above`: each channel's density over the bare film. Each as `w h` then f32 RGB (linear Rec. 2020 but `above`).

use oni_pipeline::conversion::{self, toned};
use oni_pipeline::film::{bake, cie, dyes};
use oni_render::{Stage47, cpu};
use oni_settings::{ConversionMode, PipelineSettings};
use std::io::Write;
use std::path::Path;

type Px = [f32; 3];

fn write(out: &Path, name: &str, w: usize, h: usize, px: &[Px]) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}.f32"))).unwrap());
    writeln!(f, "{w} {h}").unwrap();
    for p in px {
        for v in p {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
    }
}

/// A former profile's dyes as the scan's camera reads them, as `dyes::dyes` derives a shipped one's.
fn former(path: &str) -> &'static dyes::Dyes {
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).expect("the profile")).expect("json");
    let data = &json["data"];
    let row = |v: &serde_json::Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?])
    };
    let measured: Vec<(usize, [f64; 4])> = (0..cie::WAVELENGTHS)
        .filter_map(|l| {
            let [c, m, y] = row(&data["channel_density"][l])?;
            Some((l, [c, m, y, data["base_density"][l].as_f64()?]))
        })
        .collect();
    let weights = bake::camera_weights(&cie::D50);
    let to_rgb = bake::scan_matrix(&cie::D50);
    let (base, _) = dyes::read(&measured, &weights, [0.0; 3]);
    let (_, slope) = dyes::read(&measured, &weights, [1.0; 3]);
    let narrow = |m: &bake::M3| m.map(|r| r.map(|v| v as f32));
    Box::leak(Box::new(dyes::Dyes {
        dyes: measured.iter().map(|(_, d)| d.map(|v| v as f32)).collect(),
        weights: measured.iter().map(|(l, _)| weights[*l].map(|v| v as f32)).collect(),
        bare: bake::apply(&to_rgb, base.map(|v| 10f64.powf(-v))).map(|v| v as f32),
        to_camera: narrow(&bake::inverse(&to_rgb)),
        base: base.map(|v| v as f32),
        matrix: narrow(&bake::inverse(&slope)),
    }))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (out, side) = (Path::new(&args[0]), args[1].parse::<usize>().expect("side"));
    let spektra = (args[2] != "-").then(|| former(&args[2]));
    let split = args.iter().position(|a| a == "--").expect("-- before the scans");
    let matrices: Vec<(String, [[f32; 3]; 3])> = args[3..split]
        .iter()
        .map(|a| {
            let (name, values) = a.split_once('=').expect("name=nine values");
            let v: Vec<f32> = values.split(',').map(|x| x.parse().expect("a number")).collect();
            (name.to_string(), std::array::from_fn(|r| std::array::from_fn(|c| v[3 * r + c])))
        })
        .collect();
    std::fs::create_dir_all(out).unwrap();
    for raw in &args[split + 1..] {
        let raw = Path::new(raw);
        let stem = raw.file_stem().unwrap().to_string_lossy().to_string();
        let owner = oni_settings::sidecar::read(raw, true).expect("a sidecar").settings;
        // The frame's reading and its roll's windows, everything past them at rest on ONI's own print.
        let rest = PipelineSettings {
            gains: owner.gains,
            levels: owner.levels,
            density_ceiling: owner.density_ceiling,
            dye_set: owner.dye_set,
            print_paper: None,
            ..PipelineSettings::rest(ConversionMode::Negative, true)
        };
        let (d, _) = oni_decode::decode_for(raw, 1.0, oni_decode::HALF).expect("a decode");
        let scale = d.width.max(d.height) as f64 / side as f64;
        let (w, h) = ((d.width as f64 / scale).round() as usize, (d.height as f64 / scale).round() as usize);
        let capture = (-owner.capture_stops).exp2() as f64;
        let mut scan = vec![[0f32; 3]; w * h];
        for y in 0..h {
            let (y0, y1) = ((y as f64 * scale) as usize, (((y + 1) as f64 * scale) as usize).min(d.height));
            for x in 0..w {
                let (x0, x1) = ((x as f64 * scale) as usize, (((x + 1) as f64 * scale) as usize).min(d.width));
                let mut s = [0f64; 3];
                for yy in y0..y1 {
                    for p in &d.pixels[yy * d.width + x0..yy * d.width + x1] {
                        for c in 0..3 {
                            s[c] += f64::from(p[c]);
                        }
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)) as f64;
                scan[y * w + x] = s.map(|v| (v / n * capture) as f32);
            }
        }
        let off = conversion::stage47(&PipelineSettings { separation: 0.0, ..rest.clone() });
        assert!(off.dyes.is_none() && off.negative.is_none() && off.paper.is_none());
        let render = |k: &Stage47, px: &[Px]| -> Vec<Px> {
            px.iter()
                .map(|p| {
                    let e = cpu::stage47([p[0], p[1], p[2], 1.0], k);
                    [e[0], e[1], e[2]]
                })
                .collect()
        };
        let lv = [off.red, off.green, off.blue];
        let density = |p: &Px| -> Px { std::array::from_fn(|c| -((p[c] + off.pedestal) * off.gain[c]).max(off.log_guard).log10()) };
        write(out, &format!("{stem}-above"), w, h, &scan.iter().map(|p| std::array::from_fn(|c| density(p)[c] - lv[c][0])).collect::<Vec<Px>>());
        write(out, &format!("{stem}-off"), w, h, &render(&off, &scan));
        write(out, &format!("{stem}-now"), w, h, &render(&conversion::stage47(&PipelineSettings { separation: 1.0, ..rest.clone() }), &scan));
        if let Some(sp) = spektra {
            let black = rest.levels.windows().map(|w| w.black);
            let old = Stage47 { dyes: Some(toned(sp.separation(black, 1.0), &rest.levels, off.log_guard)), ..off };
            write(out, &format!("{stem}-old"), w, h, &render(&old, &scan));
        }
        // Past the matrix the kernel reads the mended density as light, at unit gain and no pedestal.
        let after = Stage47 { gain: [1.0; 3], pedestal: 0.0, ..off };
        for (name, m) in &matrices {
            let mended: Vec<Px> = scan
                .iter()
                .map(|p| {
                    let d = density(p);
                    let share: Px = std::array::from_fn(|c| (d[c] - lv[c][0]) / (lv[c][1] - lv[c][0]));
                    std::array::from_fn(|c| {
                        let s: f32 = (0..3).map(|k| m[c][k] * share[k]).sum();
                        10f32.powf(-(lv[c][0] + s * (lv[c][1] - lv[c][0])))
                    })
                })
                .collect();
            write(out, &format!("{stem}-{name}"), w, h, &render(&after, &mended));
        }
        println!("{stem}: {w}x{h} gains {:?} windows {:?} dyes {:?}", off.gain, lv, rest.dye_set);
    }
}
