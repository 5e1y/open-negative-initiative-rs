//! Drawings turned into the plots' Slint models, and the colours their lines take.

use super::*;

pub(super) fn ink(i: Ink) -> PlotInk {
    match i {
        Ink::Guide => PlotInk::Guide,
        Ink::GuideFaint => PlotInk::GuideFaint,
        Ink::Marker => PlotInk::Marker,
        Ink::Separator => PlotInk::Separator,
        Ink::Active => PlotInk::Active,
        Ink::Idle => PlotInk::Idle,
        Ink::Track => PlotInk::Track,
        Ink::Primary => PlotInk::Primary,
        Ink::Canvas => PlotInk::Canvas,
        Ink::Black => PlotInk::Black,
        Ink::White => PlotInk::White,
        Ink::Red => PlotInk::Red,
        Ink::Green => PlotInk::Green,
        Ink::Blue => PlotInk::Blue,
        Ink::GreyQuarter => PlotInk::GreyQuarter,
        Ink::GreyHalf => PlotInk::GreyHalf,
        Ink::GreyThreeQuarters => PlotInk::GreyThreeQuarters,
        Ink::Band(b) => [
            PlotInk::BandRed,
            PlotInk::BandOrange,
            PlotInk::BandYellow,
            PlotInk::BandGreen,
            PlotInk::BandAqua,
            PlotInk::BandBlue,
            PlotInk::BandPurple,
            PlotInk::BandMagenta,
        ][b.min(7)],
    }
}

pub(super) fn line(l: &Line) -> PlotLine {
    PlotLine { commands: l.commands.as_str().into(), ink: ink(l.ink), width: l.width as f32, opacity: l.opacity as f32 }
}

pub(super) fn dot(d: &Dot) -> PlotDot {
    PlotDot {
        x: d.x as f32,
        y: d.y as f32,
        size: d.size as f32,
        fill: ink(d.fill),
        edge: ink(d.edge),
        edge_width: d.edge_width as f32,
        halo: d.halo as f32,
    }
}

pub(super) fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

pub(crate) fn lines(d: &Drawing) -> ModelRc<PlotLine> {
    model(d.lines.iter().map(line).collect())
}

pub(crate) fn dots(d: &Drawing) -> ModelRc<PlotDot> {
    model(d.dots.iter().map(dot).collect())
}

/// `Color(hue:saturation:brightness:)`, the band slider's spectrum ends.
pub fn hsb(hue: f64, saturation: f64, brightness: f64) -> (u8, u8, u8) {
    let h = (hue.rem_euclid(1.0)) * 6.0;
    let c = brightness * saturation;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = brightness - c;
    let byte = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (byte(r), byte(g), byte(b))
}

pub(super) fn hue_end(band: Band, offset: f32) -> Color {
    let degrees = f64::from(band.centre() + offset).rem_euclid(360.0);
    let (r, g, b) = hsb(degrees / 360.0, 0.85, 1.0);
    Color::from_rgb_u8(r, g, b)
}
