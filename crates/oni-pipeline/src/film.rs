//! Virtual films: a data-sheet profile baked once into the virtual negative's kernel and table, a colour negative's
//! dyes read by ONI's scanner, the neutral film's code, and the reference that reading stands on, derived as Prepare
//! measures a roll's.

pub mod bake;
pub mod cie;
pub mod dyes;
pub mod neutral;
pub mod profile;
pub mod projection;
pub mod scanner;

use bake::M3;
use oni_render::{VirtualNegative, cpu};
use oni_settings::axis::{density_axis, graduation};
use crate::baked::Baked;
use oni_settings::stocks::VirtualEntry;
use oni_settings::virtual_film::{Balance, StockReference};
use oni_settings::{ConversionMode, DyeSet, PipelineSettings, VirtualStock};
use profile::{Profile, Silver, Spectra};
use projection::Scan;

/// The scene light spektrafilm sets on a film's speed, log exposure 0: an 18.4 % grey.
pub const GREY: f32 = 0.184;

/// The ceiling a virtual negative is read on, a mosaic negative's: its scan holds no veil of its own.
pub const CEILING: f32 = density_axis::MOSAIC_REST;

/// A stock's baked kernel at rest exposure, its table, a colour negative's scan, and its own reference as derived from
/// them; `revision`, its file's content, which a table uploaded from it is kept under.
pub struct Stock {
    pub kernel: VirtualNegative,
    pub table: Vec<[f32; 4]>,
    /// A colour negative's dyes as ONI's scanner reads them; none on a slide, projected instead, or on silver, whose
    /// neutral density a white lamp reads alike on every channel.
    pub scan: Option<Scan>,
    /// How each channel of its reading sees each layer's dye about a mid grey, each row summing to one: what mixes the
    /// layers' grains; each channel its own layer where no scan reads the film.
    pub gaze: [[f32; 3]; 3],
    pub reference: StockReference,
    pub revision: u64,
}

/// Each channel its own layer.
pub const OWN_LAYERS: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// How each channel of the reading `projection` and `table` make sees each dye about `amounts`: the rise of its density
/// for a small rise of each dye, each row laid to sum to one so a grey's grain stays grey.
pub fn gaze(projection: &oni_render::Projection, table: &[[f32; 4]], amounts: [f64; 3]) -> [[f32; 3]; 3] {
    const STEP: f64 = 0.05;
    let density = |a: [f64; 3]| {
        let out = cpu::projection([0, 1, 2].map(|k| 10f32.powf(-(a[k] as f32))).into_iter().chain([1.0]).collect::<Vec<_>>().try_into().expect("four values"), projection, table);
        [0, 1, 2].map(|c| -f64::from(out[c]).log10())
    };
    let at = density(amounts);
    let seen: [[f64; 3]; 3] = [0, 1, 2].map(|k| {
        let mut a = amounts;
        a[k] += STEP;
        let d = density(a);
        [0, 1, 2].map(|c| (d[c] - at[c]) / STEP)
    });
    std::array::from_fn(|c| {
        let row: [f64; 3] = std::array::from_fn(|k| seen[k][c]);
        let sum: f64 = row.iter().sum();
        row.map(|v| (v / sum) as f32)
    })
}

fn f32s(m: &M3) -> [[f32; 3]; 3] {
    m.map(|r| r.map(|v| v as f32))
}

/// The spectra table every stock's exposure reads, shipped unchanged.
const SPECTRA: &[u8] = include_bytes!("../resources/film/irradiance_xy_tc.npy");

/// The light a data sheet's stock is balanced for, as a spectrum: its scene's white and its layers' reference.
pub fn reference_light(stock: VirtualStock) -> &'static [f64; cie::WAVELENGTHS] {
    light_of(stock.balance()).expect("a data sheet's film names its light")
}

/// A balance's light as a spectrum; none where the scene keeps its own, which exposes no data sheet's layers.
fn light_of(balance: Balance) -> Option<&'static [f64; cie::WAVELENGTHS]> {
    match balance {
        Balance::Daylight => Some(&cie::D55),
        Balance::Tungsten => Some(&cie::INCANDESCENT),
        Balance::Scene => None,
    }
}

/// What a profile lays on the bake: each layer's sensitivity, the curves before and after the couplers on their log H
/// axis, the couplers, and a colour negative's scan.
struct Layers {
    sensitivity: Vec<[f64; 3]>,
    log_exposure: Vec<f64>,
    before: Vec<[f64; 3]>,
    after: Vec<[f64; 3]>,
    couplers: M3,
    scan: Option<Scan>,
}

/// A colour film: three layers developed through `couplers`, a reversal film's from its first developer's negative, a
/// negative's scanned; a neutral exposes the three at the same log exposure, no layer offset (owner's call).
fn colour(profile: Profile, couplers: M3, reversal: bool) -> Layers {
    let develop = if reversal { bake::curves_falling } else { bake::curves_through };
    let (before, after) = develop(&profile.log_exposure, &profile.density_curves, &couplers);
    let scan = (!reversal).then(|| scanner::scan(&profile));
    Layers { sensitivity: bake::sensitivity(&profile), log_exposure: profile.log_exposure, before, after, couplers, scan }
}

/// A black and white negative: its one layer on the three channels, no coupler.
fn silver(profile: Silver) -> Layers {
    let curve: Vec<[f64; 3]> = profile.density_curve.iter().map(|d| [*d; 3]).collect();
    Layers {
        sensitivity: profile.log_sensitivity.iter().map(|v| [10f64.powf(*v); 3]).collect(),
        log_exposure: profile.log_exposure,
        before: curve.clone(),
        after: curve,
        couplers: [[0.0; 3]; 3],
        scan: None,
    }
}

/// A film's layers as its profile holds them, a colour one's developed through `couplers`, or why the profile does not read.
fn layers(profile: &str, is_silver: bool, reversal: bool, couplers: M3) -> Result<Layers, String> {
    if is_silver {
        return Ok(silver(Silver::read(profile.as_bytes())?));
    }
    Ok(colour(Profile::read(profile.as_bytes())?, couplers, reversal))
}

/// The coupler matrix a film develops through: its file's own, else the common one.
pub fn couplers(entry: &VirtualEntry) -> M3 {
    entry.couplers.unwrap_or(bake::COUPLERS)
}

/// `entry` baked through `couplers` in place of its own, its reference derived again.
pub fn bake_through(entry: &VirtualEntry, couplers: M3) -> Result<Stock, String> {
    let layers = layers(&entry.profile, entry.silver, entry.reversal, couplers)?;
    Ok(stock_of(layers, entry))
}

/// The stock `layers` bake at `entry`'s balance, its reference derived on its scan.
fn stock_of(layers: Layers, entry: &VirtualEntry) -> Stock {
    let (kernel, table) = baked(&layers, entry.balance);
    let gaze = gaze_of(&layers);
    let mut stock = Stock { kernel, table, scan: layers.scan, gaze, reference: unread(), revision: entry.revision };
    if !entry.reversal {
        stock.reference = derived(&stock);
    }
    stock
}

/// How a colour negative's scan sees its layers' dyes at a mid grey's amounts, each layer exposed at log 0.
fn gaze_of(layers: &Layers) -> [[f32; 3]; 3] {
    let Some(scan) = &layers.scan else { return OWN_LAYERS };
    let mid = std::array::from_fn(|c| bake::interpolate(0.0, &layers.log_exposure, &layers.after, c));
    gaze(&scan.projection, &scan.table, mid)
}

/// The kernel and table of a film's `layers` lit at `balance`.
fn baked(layers: &Layers, balance: Balance) -> (VirtualNegative, Vec<[f32; 4]>) {
    let spectra = Spectra::read(SPECTRA).expect("the shipped spectra read");
    let white = light_of(balance).expect("a data sheet's film names its light, checked as its file is taken");
    let light = bake::light_table(&spectra, &layers.sensitivity);
    let light = bake::balanced(light, spectra.nodes, &bake::to_film(white), f64::from(GREY));
    let n = spectra.nodes;
    let le = &layers.log_exposure;
    let m = le.len();
    let kernel = VirtualNegative {
        to_film: f32s(&bake::to_film(white)),
        exposure: 1.0,
        couplers: f32s(&layers.couplers),
        log_low: le[0] as f32,
        log_span: (le[m - 1] - le[0]) as f32,
        tc_nodes: n,
        curve_nodes: m,
    };
    let (w, h) = kernel.table_size();
    let mut table = vec![[0.0f32; 4]; w * h];
    for i in 0..n {
        for j in 0..n {
            let v = light[i * n + j];
            table[j * w + i] = [v[0] as f32, v[1] as f32, v[2] as f32, 0.0];
        }
    }
    for (row, curve) in [&layers.before, &layers.after].iter().enumerate() {
        for (i, d) in curve.iter().enumerate() {
            table[(n + row) * w + i] = [d[0] as f32, d[1] as f32, d[2] as f32, 0.0];
        }
    }
    (kernel, table)
}

fn bake(entry: &VirtualEntry) -> Stock {
    let layers = layers(&entry.profile, entry.silver, entry.reversal, couplers(entry)).expect("a stock's profile reads once its file is taken");
    stock_of(layers, entry)
}

/// The stock baked at its first use, never before: a digital photo on another stock costs nothing. An edited file
/// bakes anew; a key no file defines bakes the default stock.
pub fn stock(stock: VirtualStock) -> &'static Stock {
    static STOCKS: Baked<(VirtualStock, u64), Stock> = Baked::new();
    let entry = stock.entry();
    STOCKS.get((entry.stock, entry.revision), || bake(&entry))
}

/// The reference a film's `profile` derives at `balance`, what its file leaving it out takes; or why the profile does
/// not read or bakes no finite reference.
pub fn derive(profile: &str, balance: Balance, is_silver: bool, reversal: bool, couplers: M3) -> Result<StockReference, String> {
    if light_of(balance).is_none() {
        return Err("a data sheet's film is balanced for daylight or tungsten".into());
    }
    let layers = layers(profile, is_silver, reversal, couplers)?;
    if bake::folds(&layers.log_exposure, &layers.before, &layers.couplers) {
        return Err("its couplers inhibit a layer faster than its exposure rises, folding its curve back".into());
    }
    let (kernel, table) = baked(&layers, balance);
    let gaze = gaze_of(&layers);
    let stock = Stock { kernel, table, scan: layers.scan, gaze, reference: unread(), revision: 0 };
    let r = if reversal { unread() } else { derived(&stock) };
    if !r.stops.iter().chain(&r.black).chain(&r.white).all(|v| v.is_finite()) {
        return Err("its profile bakes a reference that is not finite".into());
    }
    Ok(r)
}

impl Stock {
    /// The kernel at `stops` of film exposure.
    pub fn kernel_at(&self, stops: f32) -> VirtualNegative {
        VirtualNegative { exposure: stops.exp2(), ..self.kernel.clone() }
    }

    /// A developed film's layers, each amount coded as `10^-amount`, as the pipeline reads them: a colour negative's
    /// scan, a slide's or silver's as they are.
    pub fn scanned(&self, developed: [f32; 4]) -> [f32; 4] {
        match &self.scan {
            Some(s) => cpu::projection(developed, &s.projection, &s.table),
            None => developed,
        }
    }

    /// A lit scene pixel shot on the film `stops` over its rest, developed and read as the pipeline reads it.
    pub fn shot(&self, lit: [f32; 4], stops: f32) -> [f32; 4] {
        self.scanned(cpu::virtual_negative(lit, &self.kernel_at(stops), &self.table))
    }
}

/// A lit scene pixel shot on `stock` `stops` over its rest, developed and read as the pipeline reads it: a data sheet's
/// stock, or the neutral film's code.
pub fn shot(stock: VirtualStock, lit: [f32; 4], stops: f32) -> [f32; 4] {
    match neutral::kernel(stock) {
        Some(k) => cpu::neutral_negative(lit, &neutral::exposed(k, stops)),
        None => self::stock(stock).shot(lit, stops),
    }
}

/// The reference a neutral film's `profile` derives, as its file leaving it out takes; or why the profile does not read.
pub fn derive_neutral(profile: &str) -> Result<StockReference, String> {
    let k = neutral::read(profile)?;
    Ok(derived_on(|scene| cpu::neutral_negative([scene, scene, scene, 1.0], &k)))
}

/// A scan's density on the negative's axis at `stops` of gain, as stage 4 reads it.
pub fn density(scan: f32, stops: f32) -> f32 {
    -((scan + density_axis::pedestal(CEILING)) * stops.exp2()).max(density_axis::log_guard(CEILING)).log10()
}

/// What a scene grey prints at on a plain RAW's render at rest, a slide's: where a legacy print lays its reference grey.
pub fn plain_grey() -> f32 {
    let k = crate::conversion::stage47(&PipelineSettings::rest(ConversionMode::Positive, true));
    cpu::stage47([GREY, GREY, GREY, 1.0], &k)[1]
}

/// The negative at rest read on `reference`, as a digital photo on that stock opens; a black and white film's read as a
/// monochrome roll's is.
pub fn on_reference(stock: VirtualStock, reference: &StockReference) -> PipelineSettings {
    read_on(stock.is_silver(), stock.dye_set(), reference)
}

fn read_on(is_silver: bool, dyes: DyeSet, reference: &StockReference) -> PipelineSettings {
    let mode = if is_silver { ConversionMode::Monochrome } else { ConversionMode::Negative };
    let mut s = PipelineSettings::rest(mode, true);
    s.density_ceiling = CEILING;
    s.gains.stops = reference.stops;
    s.levels = reference.levels();
    s.dye_set = dyes;
    s
}

/// A reversal film's reference: its slide is projected, not read as a negative, so its windows stand at a negative's
/// rest and no gain lies on it.
fn unread() -> StockReference {
    let w = oni_settings::levels::LevelsSet::neutral(ConversionMode::Negative, CEILING).windows();
    let fraction = |d: f32| graduation::fraction(d, ConversionMode::Negative);
    StockReference { stops: [0.0; 3], black: w.map(|l| fraction(l.black)), white: w.map(|l| fraction(l.white)) }
}

/// The light a digital photo's decode lays its sensor's saturation on: a 5000 K neutral at the white level, `2^stops` of
/// the camera RAW default, the BaselineExposure every file naming none takes.
pub fn saturation() -> f32 {
    (oni_settings::white::DEFAULT_BASELINE_EV as f32).exp2()
}

/// The stock's own reference on its scan: gains that make its bare film grey, each channel's black on the bare film, no
/// light, and its white on the sensor's saturation, nothing solved on greys (owner's call).
pub fn derived(stock: &Stock) -> StockReference {
    derived_on(|scene| stock.shot([scene, scene, scene, 1.0], 0.0))
}

/// The reference of the film whose `scan` reads a grey scene of each light.
fn derived_on(scan: impl Fn(f32) -> [f32; 4]) -> StockReference {
    let (bare, white) = (scan(0.0), scan(saturation()));
    let stops = [bare[1] / bare[0], 1.0, bare[1] / bare[2]].map(f32::log2);
    let fraction = |d: f32| graduation::fraction(d, ConversionMode::Negative);
    let black = std::array::from_fn(|c| fraction(density(bare[c], stops[c])));
    let white = std::array::from_fn(|c| fraction(density(white[c], stops[c])));
    StockReference { stops, black, white }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|c| (a[c] - b[c]).abs() < 1e-5)
    }

    #[test]
    fn each_stocks_reference_is_its_profiles_derivation_and_prints_the_sensors_saturation_as_white() {
        let derived = |kind: VirtualStock| match kind.entry().neutral {
            true => derive_neutral(&kind.entry().profile).unwrap(),
            false => stock(kind).reference,
        };
        for kind in VirtualStock::all() {
            println!("{kind:?}: {:?}", derived(kind));
        }
        // A reversal film is projected, never read as a negative: its reference is the windows' rest.
        for kind in VirtualStock::all().into_iter().filter(|k| k.entry().reversal) {
            assert!(stock(kind).reference == unread() && kind.reference() == unread(), "{kind:?}");
        }
        assert!(VirtualStock::NEUTRAL.entry().neutral, "the neutral film is among them");
        for kind in VirtualStock::all().into_iter().filter(|k| !k.entry().reversal) {
            let (derived, held) = (derived(kind), kind.reference());
            assert!(close(derived.stops, held.stops) && close(derived.black, held.black) && close(derived.white, held.white), "{derived:?} {held:?}");
            let shot = |scene: f32| cpu::stage47(shot(kind, [scene, scene, scene, 1.0], 0.0), &crate::conversion::stage47(&on_reference(kind, &held)));
            let white = shot(saturation());
            assert!((0..3).all(|c| (white[c] - 1.0).abs() < 2e-3), "{kind:?}: {white:?}");
            // Twin: a scene a stop over the sensor's saturation, past what a decode holds, prints past that white.
            assert!(shot(2.0 * saturation())[1] > 1.0 + 1e-3, "{kind:?}: {:?}", shot(2.0 * saturation()));
        }
    }
}
