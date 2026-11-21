//! Virtual films: a spektrafilm profile baked once into the virtual negative's kernel and table, and
//! the reference its scan is read on, derived from the profile as Prepare the Roll measures a roll's.

pub mod bake;
pub mod cie;
pub mod dyes;
pub mod profile;

use bake::M3;
use oni_render::{VIRTUAL_NEGATIVE_BANDS, VirtualNegative, cpu};
use oni_settings::axis::{density_axis, graduation};
use oni_settings::virtual_film::StockReference;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use profile::{Profile, Spectra};
use std::sync::LazyLock;

/// The scene light spektrafilm sets on a film's speed, log exposure 0: an 18.4 % grey.
pub const GREY: f32 = 0.184;

/// The ceiling a virtual negative is read on, a mosaic negative's: its scan holds no veil of its own.
pub const CEILING: f32 = density_axis::MOSAIC_REST;

/// A stock's baked kernel at rest exposure, its table, and its own reference as derived from them.
pub struct Stock {
    pub kernel: VirtualNegative,
    pub table: Vec<[f32; 4]>,
    pub reference: StockReference,
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

/// What a scene grey prints at on a plain RAW's render at rest, a slide's: where a film's reference prints it.
pub fn plain_grey() -> f32 {
    let k = crate::conversion::stage47(&PipelineSettings::rest(ConversionMode::Positive, true));
    cpu::stage47([GREY, GREY, GREY, 1.0], &k)[1]
}

/// The negative at rest read on `reference`, its stock's dyes unmixed, as a digital photo on that stock opens.
pub fn on_reference(stock: VirtualStock, reference: &StockReference) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.density_ceiling = CEILING;
    s.gains.stops = reference.stops;
    s.levels = reference.levels();
    s.dye_set = stock.dye_set();
    s
}

/// The stock's own reference: gains that make its bare film grey, each channel's black on the bare film,
/// and its white where a scene grey, entering every window at one share, prints on the negative's print at
/// rest as a plain RAW's render prints it.
fn derived(stock: VirtualStock, kernel: &VirtualNegative, table: &[[f32; 4]]) -> StockReference {
    let scan = |scene: f32| cpu::virtual_negative([scene, scene, scene, 1.0], kernel, table);
    let (bare, grey) = (scan(0.0), scan(GREY));
    let stops = [bare[1] / bare[0], 1.0, bare[1] / bare[2]].map(f32::log2);
    let fraction = |d: f32| graduation::fraction(d, ConversionMode::Negative);
    let black: [f32; 3] = std::array::from_fn(|c| fraction(density(bare[c], stops[c])));
    let at = |share: f32| {
        let white = std::array::from_fn(|c| black[c] + (fraction(density(grey[c], stops[c])) - black[c]) / share);
        StockReference { stops, black, white }
    };
    // Placed with no unmix, as Prepare the Roll measures a roll: the dyes' tones keep its greys.
    let printed = |share: f32| cpu::stage47(grey, &crate::conversion::stage47(&PipelineSettings { separation: 0.0, ..on_reference(stock, &at(share)) }))[1];
    let target = plain_grey();
    let (mut low, mut high) = (0.05f32, 1.0f32);
    let rising = printed(high) > printed(low);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if (printed(mid) < target) == rising { low = mid } else { high = mid }
    }
    at(0.5 * (low + high))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|c| (a[c] - b[c]).abs() < 1e-5)
    }

    #[test]
    fn each_stocks_reference_is_its_profiles_derivation_and_prints_a_grey_as_a_plain_raw() {
        for kind in VirtualStock::ALL {
            let (derived, held) = (stock(kind).reference, kind.reference());
            println!("{kind:?}: {derived:?}");
            assert!(close(derived.stops, held.stops) && close(derived.black, held.black) && close(derived.white, held.white), "{derived:?} {held:?}");
            let grey = cpu::virtual_negative([GREY, GREY, GREY, 1.0], &stock(kind).kernel, &stock(kind).table);
            let printed = cpu::stage47(grey, &crate::conversion::stage47(&on_reference(kind, &held)));
            assert!((0..3).all(|c| (printed[c] - plain_grey()).abs() < 2e-3), "{printed:?} {}", plain_grey());
            // Twin: the white placed for a print resting at no exposure prints the grey brighter.
            let mut unlit = on_reference(kind, &held);
            unlit.print.exposure = 0.0;
            assert!((cpu::stage47(grey, &crate::conversion::stage47(&unlit))[1] - plain_grey()).abs() > 0.02);
        }
    }
}
