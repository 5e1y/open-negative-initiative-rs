//! The Scanner face: the measured Frontier's table from density to display sRGB with its warmth looks,
//! and the ONI machine, Pro's conversion on a resting Lighting moved by the Frontier's measured clicks.

use crate::conversion::{M3, IDENTITY, separation_matrix};
use oni_render::{ColorCube, ScannerDensity, ScannerOutput, cpu};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::blocks::SeparationFamily;
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::scanner::ScannerClicks;
use oni_settings::tone::Print;
use oni_settings::PipelineSettings;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// The separation the measured machine runs on the roll's own family.
pub const SEPARATION_DOSE: f32 = 0.6;
/// The table reads density stretched by one gain over the roll's green window, set on a measured roll.
pub const WINDOW_SCALE: f32 = 1.3;
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

    /// A count is half the machine's measured click: an odd one reads halfway between its neighbours.
    pub fn offset(&self, clicks: &ScannerClicks) -> [f32; 3] {
        let mut sum = [0.0f32; 3];
        for (key, count) in [("D", clicks.d), ("C", clicks.c), ("M", clicks.m), ("Y", clicks.y)] {
            let measured = |n: i64| -> Option<[f32; 3]> {
                let v = self.table.clicks.get(key)?.get(&n.to_string())?;
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            };
            let (low, high) = (count.div_euclid(2), count.div_euclid(2) + count.rem_euclid(2));
            if let (Some(a), Some(b)) = (measured(low), measured(high)) {
                for c in 0..3 {
                    sum[c] += (a[c] + b[c]) / 2.0;
                }
            }
        }
        sum
    }

    /// The warmth look for a count of TEMP and its share, `None` at zero.
    pub fn warmth(&self, count: i64) -> Option<(&ColorCube, f32)> {
        if count == 0 {
            return None;
        }
        let (cube, look) = if count > 0 {
            (&self.warmth_plus, &self.table.looks["warmthPlus"])
        } else {
            (&self.warmth_minus, &self.table.looks["warmthMinus"])
        };
        Some((cube, count.unsigned_abs() as f32 / look.per_count))
    }
}

/// `ScannerRender.renders`: the face, the machine; the resources ship inside the crate.
pub fn renders(s: &PipelineSettings) -> bool {
    s.face == Face::Scanner && s.scanner_machine == ScannerMachine::Frontier && crate::frontier_table::current().is_none()
}

/// `ScannerRender.densityGain`: one gain for all three channels over the roll's green window.
pub fn density_gain(s: &PipelineSettings) -> f32 {
    WINDOW_SCALE / (s.levels.green.white - s.levels.green.black).max(Print::MINIMUM_SPAN)
}

/// `ScannerRender.median`: the roll's green median on all three channels, else 0.6 of green's window.
pub fn median(s: &PipelineSettings) -> [f32; 3] {
    let window = graduation::window(s.mode);
    if let Some(median) = s.roll_reference.as_ref().and_then(|r| r.median) {
        return [window.start() + median[1] * (window.end() - window.start()); 3];
    }
    let g = s.levels.green;
    [g.black + 0.6 * (g.white - g.black); 3]
}

/// `ScannerRender.separation`: the family's plain matrix at the machine's dose, no chroma kept.
pub fn separation(s: &PipelineSettings) -> M3 {
    match (s.rgb_passes.is_none(), s.separation_family.or(SeparationFamily::resting(s.mode))) {
        (true, Some(family)) => separation_matrix(family, SEPARATION_DOSE),
        _ => IDENTITY,
    }
}

/// The density kernel's arguments for these settings on the Frontier.
pub fn density(s: &PipelineSettings) -> ScannerDensity {
    let m = &*MACHINE;
    let offset = m.offset(&s.scanner);
    let neutral = m.neutral();
    let unmix = separation(s);
    let (reached_low, reached_high) = m.reached();
    ScannerDensity {
        gain: s.gains.stops.map(f32::exp2),
        pedestal: density_axis::pedestal(s.density_ceiling),
        log_guard: density_axis::log_guard(s.density_ceiling),
        median: median(s),
        sep_r: unmix[0],
        sep_g: unmix[1],
        sep_b: unmix[2],
        scale: density_gain(s),
        shift: std::array::from_fn(|c| neutral[c] + offset[c]),
        reached_low,
        reached_high,
        low: m.low(),
        span: m.span(),
        invert: s.mode.invert_flag(),
        dmax: pipeline::DMAX,
    }
}

/// `OniLook.gainStops`: the Frontier's density offset as stops of gain, more density less light.
pub fn gain_stops(s: &PipelineSettings) -> [f32; 3] {
    let offset = MACHINE.offset(&s.scanner);
    let sign = if s.mode.invert_flag() > 0.5 { -1.0 } else { 1.0 };
    offset.map(|o| sign * o / 2f32.log10())
}

/// `onOniMachine`: Pro's conversion with Lighting and Color Density at rest, but for the balance's
/// filters and the face's Contrast, the gains moved by the clicks; `None` off the ONI machine.
pub fn on_oni_machine(s: &PipelineSettings) -> Option<PipelineSettings> {
    // A hand's Frontier look reads ONI's inversion, as ONI's own look does.
    let hand_frontier = s.scanner_machine == ScannerMachine::Frontier && crate::frontier_table::current().is_some();
    if s.face != Face::Scanner || (s.scanner_machine != ScannerMachine::Oni && !hand_frontier) {
        return None;
    }
    let mut out = s.as_scanner();
    out.print = Print { contrast: s.scanner_finish.contrast, ..Print::default() };
    if let Some(full) = s.balance_filters {
        out.print.set_filters(full.map(|f| f * s.balance_strength));
    }
    out.density = 0.0;
    let stops = gain_stops(s);
    out.gains.stops = std::array::from_fn(|c| out.gains.stops[c] + stops[c]);
    Some(out)
}

/// `ScannerRender.replica`: one transmittance through the Frontier on the CPU, as the GPU renders it;
/// `None` off the Frontier.
pub fn replica(t: [f32; 3], s: &PipelineSettings) -> Option<[f32; 3]> {
    if s.scanner_machine != ScannerMachine::Frontier {
        return None;
    }
    let m = &*MACHINE;
    let code = cpu::color_cube(cpu::scanner_density([t[0], t[1], t[2], 1.0], &density(s)), &m.render);
    let (warmth, share) = m.warmth(s.scanner.w).map_or((code, 0.0), |(cube, share)| (cpu::color_cube(code, cube), share));
    let out = cpu::scanner_output(code, warmth, &ScannerOutput { warmth_share: share, lift: LIFT, contrast: s.scanner_finish.contrast });
    Some([out[0], out[1], out[2]])
}

/// `ScannerRender.checkGrey`: a mid grey's densities on a colour negative, its orange mask included.
pub const CHECK_GREY: [f32; 3] = [0.99, 1.37, 1.67];

/// `ScannerRender.checkRoll`: a roll balanced as a measured one is, the grey on its median, on the Frontier.
pub fn check_roll() -> PipelineSettings {
    use oni_settings::ConversionMode;
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.face = Face::Scanner;
    s.scanner_machine = ScannerMachine::Frontier;
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
mod tests {
    use super::*;

    /// `ScannerKey.selfCheck`'s render half: the keypad's letters read through the measured table.
    #[test]
    fn a_density_key_lightens_and_a_colour_key_takes_its_own_channel_off() {
        let roll = check_roll();
        let mid = CHECK_GREY.map(|d| 10f32.powf(-d));
        let rendered = |clicks: ScannerClicks| replica(mid, &PipelineSettings { scanner: clicks, ..roll.clone() }).unwrap();
        let rest = rendered(ScannerClicks::default());
        let (lighter, darker) = (rendered(ScannerClicks::new(2, 0, 0, 0, 0)), rendered(ScannerClicks::new(-2, 0, 0, 0, 0)));
        let (cooler, redder) = (rendered(ScannerClicks::new(0, 2, 0, 0, 0)), rendered(ScannerClicks::new(0, -2, 0, 0, 0)));
        assert!(lighter[1] > rest[1] && darker[1] < rest[1], "D + lightens: green {} against {}", lighter[1], rest[1]);
        assert!(cooler[0] < rest[0] && redder[0] > rest[0], "C + takes red off: {} against {}", cooler[0], rest[0]);
        // Twin: off the Frontier no table is read, so a check through it cannot pass by default.
        assert!(replica(mid, &PipelineSettings { scanner_machine: ScannerMachine::Oni, ..roll.clone() }).is_none());
        // And TEMP + warms the grey: more red than blue than at rest.
        let warm = rendered(ScannerClicks::new(0, 0, 0, 0, 4));
        assert!(warm[0] - warm[2] > rest[0] - rest[2], "{warm:?} against {rest:?}");
    }

    #[test]
    fn the_table_reads_and_an_odd_count_sits_between_its_neighbours() {
        let m = &*MACHINE;
        assert_eq!((m.render.cube_dimension, m.warmth_plus.cube_dimension), (27, 17));
        assert_eq!(m.offset(&ScannerClicks::default()), [0.0; 3]);
        let one = m.offset(&ScannerClicks::new(1, 0, 0, 0, 0));
        let two = m.offset(&ScannerClicks::new(2, 0, 0, 0, 0));
        assert!((one[0] - two[0] / 2.0).abs() < 1e-6 && two[0] > 0.0, "{one:?} {two:?}");
        let minus = m.offset(&ScannerClicks::new(-3, 0, 0, 0, 0));
        assert!(minus[0] < 0.0);
        // The lift is on every node of a look, never on the render.
        assert!(m.warmth_plus.cube_data.iter().all(|n| n[0] > -LIFT));
    }

    #[test]
    fn the_oni_machine_rests_lighting_and_moves_the_gains_by_the_clicks() {
        let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        s.face = Face::Scanner;
        s.print.exposure = 0.7;
        s.density = 0.4;
        s.scanner = ScannerClicks::new(2, 0, 0, 0, 0);
        s.scanner_finish.contrast = 0.2;
        let oni = on_oni_machine(&s).unwrap();
        assert_eq!((oni.print.exposure, oni.print.contrast, oni.density), (0.0, 0.2, 0.0));
        assert!(oni.gains.stops[0] < 0.0, "more density is less light on a negative");
        // Twin: the Frontier machine and Pro both leave the settings alone.
        s.scanner_machine = ScannerMachine::Frontier;
        assert!(on_oni_machine(&s).is_none() && renders(&s));
    }
}
