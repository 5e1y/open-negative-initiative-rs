//! The print replica and the Lighting catch-up judged by the kernel's scalar windows and the print it laid on them,
//! so a fit that leans on a wrong replica cannot pass by agreeing with itself. That print lays no colour density,
//! Saturation acting on the negative, so the replica is judged at Saturation's rest.

use oni_render::{Stage47, cpu};
use oni_settings::axis::density_axis;
use oni_settings::curves::CurveSet;
use oni_settings::migrate::before_1_0;
use oni_settings::migrate::lighting::{self as lighting, Curves, display, former_pixel};
use oni_settings::tone::{Print, Tone, color_density};
use oni_settings::PipelineSettings;
use serde_json::{Value, json};

type V3 = [f32; 3];

/// The print block the 1.0 replica and catch-up were fitted to, operation for operation as the kernel laid it on its
/// windows' shares while Lighting acted on the print: the RGB curve, each channel's own tone and filter, then Hunt's colour.
mod former {
    use super::V3;
    use oni_settings::tone::Print;

    const LUMA: V3 = [0.2627, 0.6780, 0.0593];
    const GAMMA: f32 = 2.2;
    const WARP: f32 = 0.889_272_9;

    fn dot(a: V3, b: V3) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn contrasted(x: f32, travel: f32) -> f32 {
        let b = travel.abs() * 15.2;
        if b <= 1e-4 {
            return x;
        }
        let mut t = x.max(0.0).min(1.0).powf(WARP);
        let (low, high) = (1.0 / (1.0 + (b * 0.5).exp()), 1.0 / (1.0 + (-b * 0.5).exp()));
        if travel > 0.0 {
            t = (1.0 / (1.0 + (-b * (t - 0.5)).exp()) - low) / (high - low);
        } else {
            let g = low + t * (high - low);
            t = (0.5 - (1.0 / g - 1.0).ln() / b).max(0.0).min(1.0);
        }
        t.powf(1.0 / WARP)
    }

    fn quarter_pass(p: f32, t: f32, high: bool) -> f32 {
        if high { p + t * 4.0 * p * p * p * (1.0 - p) } else { p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p) }
    }

    fn quartered(p: f32, travel: f32, high: bool) -> f32 {
        if travel.abs() <= 0.25 {
            return quarter_pass(p, travel, high);
        }
        let first = if travel > 0.0 { 0.25 } else { -0.25 };
        quarter_pass(quarter_pass(p, first, high), travel - first, high)
    }

    fn shaped(p_in: f32, stops: f32, contrast: f32, zones: [f32; 2]) -> f32 {
        let held = p_in.max(0.0).min(1.0);
        let mut p = contrasted(held, contrast) + (p_in - held);
        if stops != 0.0 && p > 0.0 {
            let top = p.min(1.0);
            let (g, v) = (stops.exp2(), top.powf(GAMMA));
            p = (v * g / (1.0 + v * (g - 1.0))).powf(1.0 / GAMMA) + (p - top);
        }
        if zones != [0.0, 0.0] && (0.0..=1.0).contains(&p) {
            p = quartered(quartered(p, zones[0], false), zones[1], true);
        }
        p
    }

    fn midded(p: f32, mids: f32) -> f32 {
        if mids == 0.0 || !(0.0..=1.0).contains(&p) { p } else { p + mids * 16.0 * p * p * (1.0 - p) * (1.0 - p) }
    }

    /// `print` on the windows' shares `e`, its Exposure `stops` on the curve, Hunt's colour on.
    pub fn printed(print: &Print, stops: f32, e: V3) -> V3 {
        let zones = [print.shadows, print.highlights];
        let channel = |c: usize, x: f32, own: f32| {
            let p = shaped(x, stops, print.contrast, zones);
            midded(shaped(p, own, print.channel_contrast[c], [0.0, 0.0]), print.channel_mid[c]).max(0.0).powf(GAMMA)
        };
        let filters = print.filters();
        let mut out: V3 = std::array::from_fn(|c| channel(c, e[c], filters[c]));
        if filters != [0.0; 3] {
            // The luma held is the channels' own tone's, so a filter alone is levelled out.
            let plain: V3 = std::array::from_fn(|c| channel(c, e[c], 0.0));
            let filtered = dot(out, LUMA);
            if filtered > 1e-6 {
                let f = dot(plain, LUMA) / filtered;
                out = out.map(|x| x * f);
            }
        }
        let g = (e[0] + e[1] + e[2]) / 3.0;
        let density = (-(shaped(g, stops, print.contrast, zones) - g)).exp();
        if density == 1.0 {
            return out;
        }
        let pw = out.map(|x| x.max(0.0).powf(density));
        let p = dot(pw, LUMA);
        if p > 1e-12 { pw.map(|x| x * p.powf(1.0 / density - 1.0)) } else { out }
    }
}

/// The kernel's print block on window outputs `n`, as `Print.arguments` hands it over in colour.
fn kernel_print(print: &Print, n: V3) -> V3 {
    // Windows reading `n = D` straight: black 0, white 1, gamma 1.
    let k = Stage47 { red: [0.0, 1.0, 1.0], green: [0.0, 1.0, 1.0], blue: [0.0, 1.0, 1.0], log_guard: 1e-30, ..Stage47::rest() };
    let source = n.map(|v| 10f32.powf(-v));
    former::printed(print, print.exposure, cpu::windowed([source[0], source[1], source[2], 1.0], &k))
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

fn prints() -> Vec<Print> {
    let bent = Print { exposure: 0.7, contrast: 0.23, shadows: -0.11, highlights: 0.37, ..Print::default() };
    let filtered = Print {
        red: Tone::exposure(0.31),
        green: Tone::exposure(-0.12),
        blue: Tone::exposure(-0.47),
        ..bent
    };
    let darker = Print { exposure: -0.9, contrast: -0.3, ..filtered };
    let toned = Print { channel_contrast: [0.35, -0.2, 0.1], channel_mid: [-0.15, 0.2, 0.05], ..filtered };
    let toned_alone = Print { channel_contrast: [-0.4, 0.0, 0.3], channel_mid: [0.25, -0.1, 0.0], ..bent };
    vec![Print::default(), bent, filtered, darker, toned, toned_alone]
}

fn worst_gap(replica: impl Fn(&Print, V3, f32) -> V3) -> f32 {
    let mut worst: f32 = 0.0;
    for print in prints() {
        for n in samples() {
            let want = kernel_print(&print, n);
            let got = replica(&print, n, 0.0);
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
    // Twins: a replica dropping the filters, laying a colour density the kernel does not, or dropping each channel's
    // own tone is seen.
    let unfiltered = worst_gap(|p, n, t| {
        let mut bare = *p;
        bare.set_filters([0.0; 3]);
        bare.rendered(n, t)
    });
    assert!(unfiltered > 1e-2, "{unfiltered}");
    let apart = worst_gap(|p, n, _| color_density::rendered(p.rendered(n, 0.0), 0.35));
    assert!(apart > 1e-3, "{apart}");
    let untoned = worst_gap(|p, n, t| Print { channel_contrast: [0.0; 3], channel_mid: [0.0; 3], ..*p }.rendered(n, t));
    assert!(untoned > 1e-2, "{untoned}");
    eprintln!("print against the kernel: {worst:e}; unfiltered {unfiltered:e}; apart {apart:e}; untoned {untoned:e}");
}

/// A former document's settings object, the Swift check's own: exposure and a toned balance.
fn former_document() -> (Vec<u8>, Value) {
    let text = oni_settings::document::encode_settings(&PipelineSettings::default()).unwrap();
    let mut document: Value = serde_json::from_str(&text).unwrap();
    let settings = document["settings"].as_object_mut().unwrap();
    settings.remove("print");
    settings.insert("exposure".into(), json!(1.2));
    settings.insert("toneBalance".into(), json!({"white": {"colour": [0.8, 0, -1.6]}, "shadows": {"colour": [0, 0.3, 0]}}));
    let raw = Value::Object(settings.clone());
    document["version"] = json!(before_1_0::LIGHTING_VERSION - 1);
    (serde_json::to_vec(&document).unwrap(), raw)
}

/// A grey of transmittance `t` through the kernel, windows set to `n = D / 2`, its Exposure on the curve.
fn kernel_grey(settings: &PipelineSettings, t: f32) -> V3 {
    let curve = oni_settings::migrate::departure::former_rest(settings.mode) + oni_settings::migrate::exposure::GEAR * settings.print.exposure;
    let print = Print { exposure: curve, ..settings.print };
    let ceiling = density_axis::base();
    let k = Stage47 {
        red: [0.0, 2.0, 1.0],
        green: [0.0, 2.0, 1.0],
        blue: [0.0, 2.0, 1.0],
        pedestal: density_axis::pedestal(ceiling),
        log_guard: density_axis::log_guard(ceiling),
        ..Stage47::rest()
    };
    former::printed(&print, print.exposure, cpu::windowed([t, t, t, 1.0], &k))
}

#[test]
fn a_former_document_renders_its_greys_through_the_kernel_as_it_did() {
    let (data, raw) = former_document();
    let read = oni_settings::migrate::document(&data).unwrap().document.settings;
    assert_eq!(read.print.black_point, 0.0, "the fit renders with the points at rest");
    let f = lighting::former(&raw).expect("a former document");
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
