//! The print replica and the Lighting catch-up judged by the fused kernel's own scalar reference,
//! so a fit that leans on a wrong replica cannot pass by agreeing with itself.

use oni_render::{Stage47, cpu};
use oni_settings::axis::density_axis;
use oni_settings::codec;
use oni_settings::curves::CurveSet;
use oni_settings::legacy;
use oni_settings::lighting_catch_up::{self, Curves, display, former_pixel};
use oni_settings::tone::{Print, Tone, color_density};
use oni_settings::PipelineSettings;
use serde_json::{Value, json};

type V3 = [f32; 3];

/// The kernel's print block on window outputs `n`, as `Print.arguments` hands it over in colour.
fn kernel_print(print: &Print, n: V3, travel: f32) -> V3 {
    let k = Stage47 {
        // Windows reading `n = D` straight: black 0, white 1, gamma 1.
        red: [0.0, 1.0, 1.0],
        green: [0.0, 1.0, 1.0],
        blue: [0.0, 1.0, 1.0],
        log_guard: 1e-30,
        density: color_density::factor(travel),
        print_on: 1.0,
        print_stops: print.exposure,
        print_contrast: print.contrast,
        print_zones: [print.shadows, print.highlights],
        own_stops: print.filters(),
        hunt: 1.0,
        base_stops: print.hunt_base.map_or(0.0, |b| b.exposure),
        base_contrast: print.hunt_base.map_or(0.0, |b| b.contrast),
        base_zones: print.hunt_base.map_or([0.0; 2], |b| [b.shadows, b.highlights]),
        follow: Print::FOLLOW,
        ..Stage47::rest()
    };
    let source = n.map(|v| 10f32.powf(-v));
    let out = cpu::stage47([source[0], source[1], source[2], 1.0], &k);
    [out[0], out[1], out[2]]
}

fn samples() -> Vec<V3> {
    let mut out = Vec::new();
    for g in [0.03f32, 0.12, 0.27, 0.45, 0.63, 0.81, 0.96] {
        out.push([g; 3]);
        out.push([g * 1.21, g, g * 0.83]);
        out.push([g * 0.77, g * 1.09, g * 1.18]);
    }
    out
}

fn prints() -> Vec<(Print, f32)> {
    let bent = Print { exposure: 0.7, contrast: 0.23, shadows: -0.11, highlights: 0.37, ..Print::default() };
    let filtered = Print {
        red: Tone::exposure(0.31),
        green: Tone::exposure(-0.12),
        blue: Tone::exposure(-0.47),
        ..bent
    };
    let caught_up = Print { hunt_base: Some(Tone { exposure: 0.4, contrast: 0.1, ..Tone::NEUTRAL }), ..filtered };
    vec![(Print::default(), 0.0), (bent, 0.0), (filtered, 0.35), (caught_up, -0.6)]
}

fn worst_gap(replica: impl Fn(&Print, V3, f32) -> V3) -> f32 {
    let mut worst: f32 = 0.0;
    for (print, travel) in prints() {
        for n in samples() {
            let want = kernel_print(&print, n, travel);
            let got = replica(&print, n, travel);
            for c in 0..3 {
                worst = worst.max((display(got[c]) - display(want[c])).abs());
            }
        }
    }
    worst
}

#[test]
fn the_print_replica_is_the_kernel_s_print() {
    let worst = worst_gap(|p, n, t| p.rendered(n, t));
    assert!(worst < 1e-6, "the replica parts from the kernel by {worst}");
    // Twins: a replica dropping the filters, or the Hunt factor, is seen.
    let unfiltered = worst_gap(|p, n, t| {
        let mut bare = *p;
        bare.set_filters([0.0; 3]);
        bare.rendered(n, t)
    });
    assert!(unfiltered > 1e-2, "{unfiltered}");
    let unhunted = worst_gap(|p, n, t| {
        let mut flat = *p;
        flat.hunt_base = Some(flat.master());
        let after = flat.rendered(n, 0.0);
        color_density::rendered(after, t)
    });
    assert!(unhunted > 1e-3, "{unhunted}");
    eprintln!("print against the kernel: {worst:e}; unfiltered {unfiltered:e}; unhunted {unhunted:e}");
}

/// A former document's settings object, the Swift check's own: exposure and a toned balance.
fn former_document() -> (Vec<u8>, Value) {
    let text = codec::encode_settings(&PipelineSettings::default()).unwrap();
    let mut document: Value = serde_json::from_str(&text).unwrap();
    let settings = document["settings"].as_object_mut().unwrap();
    settings.remove("print");
    settings.insert("exposure".into(), json!(1.2));
    settings.insert("toneBalance".into(), json!({"white": {"colour": [0.8, 0, -1.6]}, "shadows": {"colour": [0, 0.3, 0]}}));
    let raw = Value::Object(settings.clone());
    document["version"] = json!(legacy::LIGHTING_VERSION - 1);
    (serde_json::to_vec(&document).unwrap(), raw)
}

/// A grey of transmittance `t` through the kernel, windows set to `n = D / 2`, separation off.
fn kernel_grey(settings: &PipelineSettings, t: f32) -> V3 {
    let print = settings.print;
    let ceiling = density_axis::base();
    let k = Stage47 {
        red: [0.0, 2.0, 1.0],
        green: [0.0, 2.0, 1.0],
        blue: [0.0, 2.0, 1.0],
        pedestal: density_axis::pedestal(ceiling),
        log_guard: density_axis::log_guard(ceiling),
        density: color_density::factor(settings.density),
        print_on: 1.0,
        print_stops: print.exposure,
        print_contrast: print.contrast,
        print_zones: [print.shadows, print.highlights],
        own_stops: print.filters(),
        hunt: 1.0,
        base_stops: print.hunt_base.map_or(0.0, |b| b.exposure),
        base_contrast: print.hunt_base.map_or(0.0, |b| b.contrast),
        base_zones: print.hunt_base.map_or([0.0; 2], |b| [b.shadows, b.highlights]),
        follow: Print::FOLLOW,
        ..Stage47::rest()
    };
    let out = cpu::stage47([t, t, t, 1.0], &k);
    [out[0], out[1], out[2]]
}

#[test]
fn a_former_document_renders_its_greys_through_the_kernel_as_it_did() {
    let (data, raw) = former_document();
    let read = codec::decode(&data).unwrap().settings;
    assert_eq!(read.print.black_point, 0.0, "the fit renders with the points at rest");
    let f = lighting_catch_up::former(&raw).expect("a former document");
    let identity = Curves::new(&CurveSet::default());
    let n = |t: f32| {
        let ceiling = density_axis::base();
        -(t + density_axis::pedestal(ceiling)).max(density_axis::log_guard(ceiling)).log10() / 2.0
    };
    let bare = PipelineSettings { print: Print::default(), density: 0.0, ..read.clone() };
    let (mut worst, mut unmigrated) = (0f32, 0f32);
    for t in [0.02f32, 0.08, 0.2, 0.45] {
        let target = former_pixel([n(t); 3], &f, &identity, &identity);
        let got = kernel_grey(&read, t);
        let plain = kernel_grey(&bare, t);
        for c in 0..3 {
            worst = worst.max((display(got[c]) - display(target[c])).abs());
            unmigrated = unmigrated.max((display(plain[c]) - display(target[c])).abs());
        }
    }
    eprintln!("through the kernel: worst {worst:.4}, unmigrated {unmigrated:.4}");
    assert!(worst < 0.07, "{worst}");
    // Twin: left on Lighting's rest the same greys stand apart.
    assert!(unmigrated > 0.08, "{unmigrated}");
}
