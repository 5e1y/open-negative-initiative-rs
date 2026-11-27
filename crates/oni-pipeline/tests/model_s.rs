//! The colour negative's virtual film against the colorimetry's model S (`data/model-s-reference.json`) step by step, baked
//! on the model's own sheet curves and couplers; its twin, the same film with no inhibition between its layers, misses it.

use oni_pipeline::{conversion, film};
use oni_render::cpu;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use serde_json::Value;

/// How far the app may lie from the model: f32 and the table read bilinearly on its 192-node square, the bare film read
/// on its sheet's known bands alone.
const LOG_H: f64 = 1e-5;
const DENSITY: f64 = 1e-3;
const SHARE: f64 = 1e-3;

fn v3(v: &Value) -> [f64; 3] {
    std::array::from_fn(|c| v[c].as_f64().expect("a number"))
}

fn worst(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f64::max)
}

/// The film's widest gaps to the model on every colour, log H, density and share, developed through `stock`.
fn gaps(key: &'static str, model: &Value, stock: &film::Stock) -> [f64; 3] {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of(key));
    let k = conversion::stage47(&oni_pipeline::film::on_reference(VirtualStock::of(key), &stock.reference));
    let mut out = [0.0f64; 3];
    for (name, c) in model["colours"].as_object().expect("the colours") {
        let rgb = v3(&c["rgb_working"]).map(|v| v as f32);
        let lit = [rgb[0], rgb[1], rgb[2], 1.0];
        let exposed = cpu::virtual_exposure(lit, &stock.kernel_at(s.film().unwrap().exposure), &stock.table);
        let log_h: [f64; 3] = std::array::from_fn(|i| f64::from(exposed[i].max(1e-10)).log10());
        let scan = cpu::virtual_development(exposed, &stock.kernel, &stock.table);
        let veil = f64::from(stock.kernel.veil);
        let density: [f64; 3] = std::array::from_fn(|i| -((f64::from(scan[i]) + veil) / (1.0 + veil)).log10());
        let share = cpu::windowed(scan, &k).map(f64::from);
        let gap = [worst(log_h, v3(&c["logH"])), worst(density, v3(&c["D_total"])), worst(share, v3(&c["share"]))];
        if gap[0] > LOG_H || gap[1] > DENSITY || gap[2] > SHARE {
            println!("{key} {name}: log H {log_h:?} for {}, D {density:?} for {}, share {share:?} for {}", c["logH"], c["D_total"], c["share"]);
        }
        out = std::array::from_fn(|i| out[i].max(gap[i]));
    }
    out
}

#[test]
fn every_film_develops_and_reads_as_the_model_does() {
    let reference: Value = serde_json::from_str(include_str!("data/model-s-reference.json")).unwrap();
    for (file, model) in reference.as_object().expect("films by file") {
        let key: &'static str = match file.as_str() {
            "kodak-portra800" => "kodak-portra-800",
            "kodak-gold200" => "kodak-gold-200",
            "kodak-ektar100" => "kodak-ektar-100",
            other => Box::leak(other.to_string().into_boxed_str()),
        };
        // The film baked on the model's own inputs, its sheet's curves and the couplers the model derives from them, as
        // its file may fit others on a reference.
        let held: Vec<f64> = model["K"].as_array().unwrap().iter().flat_map(|r| r.as_array().unwrap().iter().map(|v| v.as_f64().unwrap())).collect();
        let modelled: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| held[r * 3 + c]));
        let entry = oni_settings::stocks::VirtualEntry { curves: None, ..VirtualStock::of(key).entry() };
        let baked = film::bake_through(&entry, modelled).unwrap();
        let stock = &baked;
        let veil = f64::from(stock.kernel.veil);
        let bare = stock.shot([0.0, 0.0, 0.0, 1.0], 0.0).map(|t| -((f64::from(t) + veil) / (1.0 + veil)).log10());
        assert!(worst([bare[0], bare[1], bare[2]], v3(&model["D_min_status_M"])) < DENSITY, "{key}: D-min {bare:?}");
        let gap = gaps(key, model, stock);
        println!("{key}: widest gaps to the model, log H {:.1e}, density {:.1e}, share {:.1e}", gap[0], gap[1], gap[2]);
        assert!(gap[0] < LOG_H && gap[1] < DENSITY && gap[2] < SHARE, "{key}: {gap:?}");
        // Twin: the same film with no inhibition between its layers misses the model's colours.
        let free = film::bake_through(&entry, [[0.0; 3]; 3]).unwrap();
        let off = gaps(key, model, &free);
        assert!(off[1] > 10.0 * DENSITY, "twin {key}: {off:?}");
    }
}
