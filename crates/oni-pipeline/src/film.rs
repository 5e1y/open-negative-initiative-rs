//! Virtual films: a spektrafilm profile baked once into the virtual negative's kernel and table, and
//! the reference its scan is read on, derived from the profile as Prepare the Roll measures a roll's.

pub mod bake;
pub mod cie;
pub mod profile;

use bake::M3;
use oni_render::{VIRTUAL_NEGATIVE_BANDS, VirtualNegative, cpu};
use oni_settings::axis::{density_axis, graduation};
use oni_settings::{ConversionMode, PipelineSettings, VirtualFilm, VirtualStock};
use profile::{Profile, Spectra};
use std::sync::LazyLock;

/// The scene light spektrafilm sets on a film's speed, log exposure 0: an 18.4 % grey.
pub const GREY: f32 = 0.184;

/// The ceiling a virtual negative is read on, a mosaic negative's: its scan holds no veil of its own.
pub const CEILING: f32 = density_axis::MOSAIC_REST;

/// A stock's baked kernel at rest exposure, its table, and its own reference.
pub struct Stock {
    pub kernel: VirtualNegative,
    pub table: Vec<[f32; 4]>,
    pub reference: VirtualFilm,
}

fn f32s(m: &M3) -> [[f32; 3]; 3] {
    m.map(|r| r.map(|v| v as f32))
}

/// The profile's sources, read from the files spektrafilm ships, unchanged.
fn sources(stock: VirtualStock) -> (&'static [u8], &'static [u8]) {
    match stock {
        VirtualStock::KodakPortra800 => (include_bytes!("../resources/film/kodak_portra_800.json"), include_bytes!("../resources/film/irradiance_xy_tc.npy")),
    }
}

fn bake(stock: VirtualStock) -> Stock {
    let (json, npy) = sources(stock);
    let profile = Profile::read(json).expect("a shipped profile reads");
    let spectra = Spectra::read(npy).expect("the shipped spectra read");
    let sensitivity = bake::windowed_sensitivity(&profile, &cie::D55);
    let light = bake::light_table(&spectra, &sensitivity, &cie::D55);
    let (before, after) = bake::curves(&profile);
    let scanned = bake::scanned(&profile);
    assert!(scanned.len() <= VIRTUAL_NEGATIVE_BANDS, "the uniform holds every scanned wavelength");
    let weights = bake::scan_weights(&cie::D50);
    let n = spectra.nodes;
    let m = profile.log_exposure.len();
    let le = &profile.log_exposure;
    let kernel = VirtualNegative {
        to_film: f32s(&bake::to_film(&cie::D55)),
        exposure: 1.0,
        couplers: f32s(&bake::COUPLERS),
        log_low: le[0] as f32,
        log_span: (le[m - 1] - le[0]) as f32,
        tc_nodes: n,
        curve_nodes: m,
        dyes: scanned.iter().map(|(_, d)| d.map(|v| v as f32)).collect(),
        weights: scanned.iter().map(|(l, _)| weights[*l].map(|v| v as f32)).collect(),
    };
    let (w, h) = kernel.table_size();
    let mut table = vec![[0.0f32; 4]; w * h];
    for i in 0..n {
        for j in 0..n {
            let v = light[i * n + j];
            table[j * w + i] = [v[0] as f32, v[1] as f32, v[2] as f32, 0.0];
        }
    }
    for (row, curve) in [before, after].iter().enumerate() {
        for (i, d) in curve.iter().enumerate() {
            table[(n + row) * w + i] = [d[0] as f32, d[1] as f32, d[2] as f32, 0.0];
        }
    }
    let reference = derived(stock, &kernel, &table);
    Stock { kernel, table, reference }
}

static PORTRA_800: LazyLock<Stock> = LazyLock::new(|| bake(VirtualStock::KodakPortra800));

pub fn stock(stock: VirtualStock) -> &'static Stock {
    match stock {
        VirtualStock::KodakPortra800 => &PORTRA_800,
    }
}

impl Stock {
    /// The kernel at `stops` of film exposure.
    pub fn kernel_at(&self, stops: f32) -> VirtualNegative {
        VirtualNegative { exposure: stops.exp2(), ..self.kernel.clone() }
    }
}

/// A scan's density on the negative's axis at `stops` of gain, as stage 4 reads it.
pub fn density(scan: f32, stops: f32) -> f32 {
    -((scan + density_axis::pedestal(CEILING)) * stops.exp2()).max(density_axis::log_guard(CEILING)).log10()
}

/// Where a digital photo's own render at rest windows a scene value: the share of its print it enters.
pub fn digital_share(scene: f32) -> f32 {
    let k = crate::conversion::stage47(&PipelineSettings::rest(ConversionMode::Digital, true));
    let e = k.window + ((scene + k.pedestal) * k.gain[1]).max(k.log_guard).log10();
    ((e - k.green[0]) / (k.green[1] - k.green[0]).max(1e-4)).max(0.0).powf(k.green[2])
}

/// The stock's own reference: gains that make its bare film grey, each channel's black on the bare
/// film, and its white where a scene grey enters the print where the digital photo's own render enters it.
fn derived(stock: VirtualStock, kernel: &VirtualNegative, table: &[[f32; 4]]) -> VirtualFilm {
    let scan = |scene: f32| {
        let out = cpu::virtual_negative([scene, scene, scene, 1.0], kernel, table);
        [out[0], out[1], out[2]]
    };
    let (bare, grey) = (scan(0.0), scan(GREY));
    let stops = [bare[1] / bare[0], 1.0, bare[1] / bare[2]].map(f32::log2);
    let share = digital_share(GREY);
    let fraction = |d: f32| graduation::fraction(d, ConversionMode::Negative);
    let black: [f32; 3] = std::array::from_fn(|c| fraction(density(bare[c], stops[c])));
    let white = std::array::from_fn(|c| {
        let d = fraction(density(grey[c], stops[c]));
        black[c] + (d - black[c]) / share
    });
    VirtualFilm { stock, exposure: 0.0, stops, black, white }
}
