//! An editor's drawing, as the app's own editors lay it out, turned into the scene's `PlotLine` and
//! `PlotDot` models.

use oni_app::editors::geometry::{Dot, Drawing, Ink, Line};
use slint::{ModelRc, VecModel};
use slint_interpreter::{Struct, Value};

/// The drawing's lines and dots, for the scene's `lines` and `dots`.
pub fn values(d: &Drawing) -> (Value, Value) {
    (model(d.lines.iter().map(line).collect()), model(d.dots.iter().map(dot).collect()))
}

fn model(items: Vec<Value>) -> Value {
    Value::Model(ModelRc::new(VecModel::from(items)))
}

fn line(l: &Line) -> Value {
    record([
        ("commands", Value::String(l.commands.as_str().into())),
        ("ink", ink(l.ink)),
        ("width", Value::Number(l.width)),
        ("opacity", Value::Number(l.opacity)),
    ])
}

fn dot(d: &Dot) -> Value {
    record([
        ("x", Value::Number(d.x)),
        ("y", Value::Number(d.y)),
        ("size", Value::Number(d.size)),
        ("fill", ink(d.fill)),
        ("edge", ink(d.edge)),
        ("edge-width", Value::Number(d.edge_width)),
        ("halo", Value::Number(d.halo)),
    ])
}

fn record<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Struct(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Struct>())
}

/// The design system's `Ink`, by its Slint name.
fn ink(i: Ink) -> Value {
    const BANDS: [&str; 8] = ["band-red", "band-orange", "band-yellow", "band-green", "band-aqua", "band-blue", "band-purple", "band-magenta"];
    let name = match i {
        Ink::Guide => "guide",
        Ink::GuideFaint => "guide-faint",
        Ink::Marker => "marker",
        Ink::Separator => "separator",
        Ink::Active => "active",
        Ink::Idle => "idle",
        Ink::Track => "track",
        Ink::Primary => "primary",
        Ink::Canvas => "canvas",
        Ink::Black => "black",
        Ink::White => "white",
        Ink::Red => "red",
        Ink::Green => "green",
        Ink::Blue => "blue",
        Ink::GreyQuarter => "grey-quarter",
        Ink::GreyHalf => "grey-half",
        Ink::GreyThreeQuarters => "grey-three-quarters",
        Ink::Band(b) => BANDS[b.min(7)],
    };
    Value::EnumerationValue("Ink".into(), name.into())
}
