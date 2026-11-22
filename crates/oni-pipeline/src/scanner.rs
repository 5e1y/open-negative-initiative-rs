//! The former Scanner face's machines a legacy print renders: the measured Frontier's table from density to display sRGB
//! with its warmth looks, its keys the Frontier's measured clicks laid in the gains of ONI's conversion.

use crate::conversion::{M3, IDENTITY};
use oni_render::{ColorCube, ScannerDensity, ScannerOutput, cpu};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, LegacyMachine, LegacyPrint, PipelineSettings};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// `ScannerRender.lift`: the warmth looks' moves stored raised by it, since a cube clips under zero.
pub const LIFT: f32 = 0.5;

#[derive(Deserialize)]
struct Render {
    cube: String,
    nodes: usize,
    #[serde(rename = "densityRange")]
    density_range: [f32; 2],
    reached: Option<[[f32; 3]; 2]>,
}

#[derive(Deserialize)]
struct Neutral {
    density: [f32; 3],
}

#[derive(Deserialize)]
struct Look {
    cube: String,
    nodes: usize,
    #[serde(rename = "perCount")]
    per_count: f32,
}

#[derive(Deserialize)]
struct Table {
    render: Render,
    neutral: Neutral,
    clicks: HashMap<String, HashMap<String, Vec<f32>>>,
    looks: HashMap<String, Look>,
}

/// The measured Frontier, every number read off its resources as the fit wrote it.
pub struct Machine {
    table: Table,
    pub render: ColorCube,
    pub warmth_plus: ColorCube,
    pub warmth_minus: ColorCube,
}

fn resource(name: &str) -> &'static [u8] {
    match name {
        "frontier.cube" => include_bytes!("../resources/scanner/frontier.cube"),
        "warmthPlus.cube" => include_bytes!("../resources/scanner/warmthPlus.cube"),
        "warmthMinus.cube" => include_bytes!("../resources/scanner/warmthMinus.cube"),
        other => panic!("no scanner resource {other}"),
    }
}

/// A look's moves raised by `LIFT`, so a cooling move survives the cube's clip under zero.
fn lifted(mut cube: ColorCube) -> ColorCube {
    for node in &mut cube.cube_data {
        for v in &mut node[..3] {
            *v += LIFT;
        }
    }
    cube
}

pub static MACHINE: LazyLock<Machine> = LazyLock::new(|| {
    let table: Table = serde_json::from_slice(include_bytes!("../resources/scanner/frontier.json")).expect("frontier.json");
    let cube = |name: &str, nodes: usize| ColorCube::from_bytes(nodes, resource(name)).expect("a cube of its stated size");
    let render = cube(&table.render.cube, table.render.nodes);
    let plus = &table.looks["warmthPlus"];
    let minus = &table.looks["warmthMinus"];
    let warmth_plus = lifted(cube(&plus.cube, plus.nodes));
    let warmth_minus = lifted(cube(&minus.cube, minus.nodes));
    Machine { table, render, warmth_plus, warmth_minus }
});

impl Machine {
    pub fn low(&self) -> f32 {
        self.table.render.density_range[0]
    }

    pub fn span(&self) -> f32 {
        self.table.render.density_range[1] - self.table.render.density_range[0]
    }

    pub fn neutral(&self) -> [f32; 3] {
        self.table.neutral.density
    }

    pub fn reached(&self) -> ([f32; 3], [f32; 3]) {
        self.table.render.reached.map_or(([self.low(); 3], [self.low() + self.span(); 3]), |r| (r[0], r[1]))
    }

    /// D, C, M and Y's steps as a density offset: a step is a quarter of the machine's measured click, read on the line
    /// between its neighbours.
    pub fn offset(&self, keys: [i64; 4]) -> [f32; 3] {
        const PER_CLICK: i64 = 2 * LegacyPrint::PER_POINT;
        let mut sum = [0.0f32; 3];
        for (key, steps) in ["D", "C", "M", "Y"].into_iter().zip(keys) {
            let measured = |n: i64| -> Option<[f32; 3]> {
                let v = self.table.clicks.get(key)?.get(&n.to_string())?;
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            };
            let (low, part) = (steps.div_euclid(PER_CLICK), steps.rem_euclid(PER_CLICK));
            let share = part as f32 / PER_CLICK as f32;
            if let (Some(a), Some(b)) = (measured(low), measured(low + i64::from(part > 0))) {
                for c in 0..3 {
                    sum[c] += a[c] + share * (b[c] - a[c]);
                }
            }
        }
        sum
    }

    /// The warmth look for TEMP's steps and its share, `None` at zero.
    pub fn warmth(&self, count: i64) -> Option<(&ColorCube, f32)> {
        if count == 0 {
            return None;
        }
        let (cube, look) = if count > 0 {
            (&self.warmth_plus, &self.table.looks["warmthPlus"])
        } else {
            (&self.warmth_minus, &self.table.looks["warmthMinus"])
        };
        Some((cube, count.unsigned_abs() as f32 / (look.per_count * LegacyPrint::PER_POINT as f32)))
    }
}

/// Where the table prints its grey black (under 0.3 %) and white (over 98.5 %), as green density along
/// its neutral axis: the ends ONI's window is laid on.
pub static ENDS: LazyLock<(f32, f32)> = LazyLock::new(|| {
    let m = &*MACHINE;
    let neutral = m.neutral();
    let grey = |g: f32| {
        let n = neutral.map(|c| ((c + g - neutral[1] - m.low()) / m.span()).clamp(0.0, 1.0));
        cpu::color_cube([n[0], n[1], n[2], 1.0], &m.render)[1]
    };
    let steps = (0..=550).map(|i| m.low() + m.span() * i as f32 / 550.0);
    let black = steps.clone().filter(|g| grey(*g) <= 0.003).fold(m.low(), f32::max);
    let white = steps.filter(|g| grey(*g) >= 0.985).fold(m.low() + m.span(), f32::min);
    (black, white)
});

/// The Swift machine's unmix the Look Lab's sets were fitted through: its generic rows, each normalised to
/// sum to one, dosed at 0.6; none on RGB passes, whose capture separated the dyes.
pub fn separation(s: &PipelineSettings) -> M3 {
    const ROWS: M3 = [[1.0000, -0.2706, -0.0076], [-0.0517, 1.0000, -0.0137], [-0.0228, -0.0723, 1.0000]];
    if s.rgb_passes.is_some() {
        return IDENTITY;
    }
    let full = ROWS.map(|r| r.map(|x| x / (r[0] + r[1] + r[2])));
    std::array::from_fn(|i| std::array::from_fn(|j| IDENTITY[i][j] + 0.6 * (full[i][j] - IDENTITY[i][j])))
}

/// The coordinates the Look Lab's sets were fitted in, the Swift machine's: one stretch of 1.3 over the
/// roll's green window about its median, the machine's neutral on that median.
pub fn fitted_density(s: &PipelineSettings) -> ScannerDensity {
    let m = &*MACHINE;
    let (offset, neutral) = (m.offset(keys(s)), m.neutral());
    let (window, g) = (graduation::window(s.mode), s.levels.green);
    let median = s.roll_reference.as_ref().and_then(|r| r.median).map_or(g.black + 0.6 * (g.white - g.black), |m| window.start() + m[1] * (window.end() - window.start()));
    let unmix = separation(s);
    let (reached_low, reached_high) = m.reached();
    ScannerDensity {
        gain: s.gains.stops.map(f32::exp2),
        pedestal: density_axis::pedestal(s.density_ceiling),
        log_guard: density_axis::log_guard(s.density_ceiling),
        median: [median; 3],
        sep_r: unmix[0],
        sep_g: unmix[1],
        sep_b: unmix[2],
        scale: 1.3 / (g.white - g.black).max(Print::MINIMUM_SPAN),
        shift: std::array::from_fn(|c| neutral[c] + offset[c]),
        reached_low,
        reached_high,
        low: m.low(),
        span: m.span(),
        invert: s.mode.invert_flag(),
        dmax: pipeline::DMAX,
    }
}

/// The laid legacy print's D, C, M and Y steps, none without one.
fn keys(s: &PipelineSettings) -> [i64; 4] {
    s.legacy().map_or([0; 4], |l| [l.d, l.c, l.m, l.y])
}

/// The legacy print's keys as stops of gain, more density less light; where the Frontier's look's cube prints, its rest
/// with them; none without a legacy print laid.
pub fn gain_stops(s: &PipelineSettings) -> [f32; 3] {
    let clicks = MACHINE.offset(keys(s));
    let rests = s.legacy().is_some_and(|l| l.machine == LegacyMachine::Frontier) && s.paper().is_none();
    let rest = if rests { frontier_rest() } else { [0.0; 3] };
    stops_of(std::array::from_fn(|c| clicks[c] + rest[c]), s)
}

/// A density offset of the machine's as stops of gain, more density less light.
pub fn stops_of(offset: [f32; 3], s: &PipelineSettings) -> [f32; 3] {
    let sign = if s.mode.invert_flag() > 0.5 { -1.0 } else { 1.0 };
    offset.map(|o| sign * o / 2f32.log10())
}

/// Where the Frontier's look rests, the owner's own neutral for it, in the gains before the conversion: D+0.5 M+2 Y-2
/// in the keypad's points, D, C, M and Y's steps here.
pub const FRONTIER_REST: [i64; 4] = [1, 0, 4, -4];

/// The rest as a density offset.
pub fn frontier_rest() -> [f32; 3] {
    MACHINE.offset(FRONTIER_REST)
}

/// The settings a photo on a laid legacy print renders by: its own, the gains moved by the keys; `None` without one.
pub fn on_legacy_print(s: &PipelineSettings) -> Option<PipelineSettings> {
    s.legacy()?;
    let mut out = s.clone();
    let stops = gain_stops(s);
    out.gains.stops = std::array::from_fn(|c| out.gains.stops[c] + stops[c]);
    Some(out)
}

/// `ScannerRender.replica`: one transmittance through the measured Frontier table on the CPU, as the GPU renders it;
/// `None` off a Frontier legacy print.
pub fn replica(t: [f32; 3], s: &PipelineSettings) -> Option<[f32; 3]> {
    let legacy = s.legacy().filter(|l| l.machine == LegacyMachine::Frontier)?;
    let m = &*MACHINE;
    let code = cpu::color_cube(cpu::scanner_density([t[0], t[1], t[2], 1.0], &fitted_density(s)), &m.render);
    let (warmth, share) = m.warmth(legacy.w).map_or((code, 0.0), |(cube, share)| (cpu::color_cube(code, cube), share));
    let out = cpu::scanner_output(code, warmth, &ScannerOutput { warmth_share: share, lift: LIFT, contrast: s.print.contrast });
    Some([out[0], out[1], out[2]])
}

/// `ScannerRender.checkGrey`: a mid grey's densities on a colour negative, its orange mask included.
pub const CHECK_GREY: [f32; 3] = [0.99, 1.37, 1.67];

/// `ScannerRender.checkRoll`: a roll balanced as a measured one is, the grey on its median, on the Frontier's legacy print.
pub fn check_roll() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, ..LegacyPrint::default() });
    let lift = CHECK_GREY.map(|g| g - CHECK_GREY[1]);
    s.gains.stops = lift.map(|l| l / 2f32.log10());
    let base = [0.24 - lift[0], 0.62 - lift[1], 0.92 - lift[2]];
    let grey = CHECK_GREY[1];
    let white = base.map(|b| b + 2.0 * (grey - b));
    (s.levels.red.black, s.levels.green.black, s.levels.blue.black) = (base[0], base[1], base[2]);
    (s.levels.red.white, s.levels.green.white, s.levels.blue.white) = (white[0], white[1], white[2]);
    let window = graduation::window(ConversionMode::Negative);
    let median = (grey - window.start()) / (window.end() - window.start());
    s.roll_reference = Some(oni_settings::frame::RollReference {
        id: oni_settings::Id::new(),
        stops: s.gains.stops,
        black: [0.1; 3],
        white: [0.7; 3],
        base: None,
        median: Some([median; 3]),
    });
    s
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod tests;
