//! A slide on the projector's screen, a scanned one or a digital photo shot on a reversal film: its dyes' amounts under
//! Lighting, developed again through its stock's curve, filtering the projector's lamp, then the screen's ends, the film's
//! black held where the slide at rest shows it.

use crate::film::projection::{self, Projected};
use crate::negative;
use oni_render::{ColorMatrix, DyeAmounts, HeldBlack, LIGHTNESS_HIGH, LIGHTNESS_LOW, LIGHTNESS_NODES, LUMA, Negative, cpu};
use oni_settings::tone::Print;
use oni_settings::{PipelineSettings, VirtualStock};

/// The scan's solve for `s`'s dyes: its gains put the backlight on 1, as a sprocket hole reads.
pub fn scanned(s: &PipelineSettings, stock: VirtualStock) -> DyeAmounts {
    projection::scanned(stock).at(s.gains.stops)
}

/// `s`'s Lighting and colour tools on its slide's dyes, the saturation curve reading the screen's lightness of a grey;
/// none while they rest.
pub fn lighting(s: &PipelineSettings, stock: VirtualStock) -> Option<Negative> {
    let n = negative::slide(s, stock)?;
    if n.curve.is_none() {
        return Some(n);
    }
    let shown = Negative { curve: None, ..n };
    Some(Negative { lightness: lightness(&shown, s, stock), ..n })
}

/// The screen's lightness, its luma on a display gamma, of a grey at each of `n`'s lightness nodes, as `s` shows it.
fn lightness(n: &Negative, s: &PipelineSettings, stock: VirtualStock) -> [f32; LIGHTNESS_NODES] {
    let p = projection::projected(stock);
    let ends = ends(s, stock);
    let step = (LIGHTNESS_HIGH - LIGHTNESS_LOW) / (LIGHTNESS_NODES - 1) as f32;
    let held = held(s, stock);
    std::array::from_fn(|i| {
        let grey = n.grey(LIGHTNESS_LOW + step * i as f32);
        let on_screen = |a: [f32; 4]| {
            let v = cpu::projection(a, &p.projection, &p.table);
            ends.map_or(v, |m| cpu::color_matrix(v, &m))
        };
        let v = cpu::held_black(on_screen(cpu::negative(grey, n)), on_screen(grey), &held);
        (0..3).map(|c| LUMA[c] * v[c]).sum::<f32>().max(0.0).powf(1.0 / Print::DISPLAY_GAMMA).clamp(0.0, 1.0)
    })
}

/// The screen's ends as `s` shows them: none with Projected blacks, the film's densest under the veil and the bare
/// film's white as the screen shows them; off, laid on pure black and pure white.
pub fn ends(s: &PipelineSettings, stock: VirtualStock) -> Option<ColorMatrix> {
    (!s.print.projected_blacks).then(|| projection::pure(projection::projected(stock)))
}

/// Where `s`'s screen holds its black under Lighting: the film's densest as the screen shows it, none of a slider's move
/// there, the whole from Brightness's reach above it, the light a paper's print takes Brightness whole from.
pub fn held(s: &PipelineSettings, stock: VirtualStock) -> HeldBlack {
    let p = projection::projected(stock);
    let black = if s.print.projected_blacks { (0..3).map(|c| LUMA[c] * p.black[c]).sum() } else { 0.0 };
    HeldBlack { black, reach: crate::paper::BRIGHTNESS_REACH_LIGHT }
}

/// One pixel of `s`'s slide, its dyes' amounts coded as `10^-amount`, on the screen: the reference the render runs.
pub fn shown(amounts: [f32; 4], s: &PipelineSettings, stock: VirtualStock) -> [f32; 4] {
    let p: &Projected = projection::projected(stock);
    let ends = ends(s, stock);
    let on_screen = |a: [f32; 4]| {
        let v = cpu::projection(a, &p.projection, &p.table);
        ends.map_or(v, |m| cpu::color_matrix(v, &m))
    };
    let rest = on_screen(amounts);
    lighting(s, stock).map_or(rest, |n| cpu::held_black(on_screen(cpu::negative(amounts, &n)), rest, &held(s, stock)))
}
