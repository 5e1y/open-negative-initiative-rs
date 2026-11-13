//! The interface film in 1.6: the hand lights two flat keys, wakes the colour rails, types over Temperature's
//! number and leaves; the components answer the hand themselves, the timeline answers the store as the app.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use oni_app::fields::{Block, Field};
use oni_app::slider_track::{number, position_on, rests, stepped, track_on, typed_steps};
use oni_settings::{ConversionMode, PipelineSettings};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use slint_interpreter::{ComponentInstance, Value};

use crate::film::{Demo, Hand};
use crate::photo::{Photo, plain};
use crate::stage::{SCALE, Stage};
use crate::track::{Presses, Track};

/// What the hand types over Temperature's number, then Return: short, signed, with a decimal.
pub const TYPED: &str = "-1.2";
/// The photograph's strip, as the before cuts it: the front table and its lamp, past the film's black
/// edge above and the holder's notches below (6272 × 4179 reduced to 1600 × 1066, from row 28).
pub const CUT: plain::Cut = plain::Cut { side: 1600.0, centre: 0.24, top: 28 };
/// The film's length, from the wipe's last frame: the beat sheet's 2.9 s to 9.6 s.
const SECONDS: f32 = 6.7;
/// A character every 100 ms, as a hand types a short figure.
const STROKE: f32 = 0.1;

/// Rectangles in points: x, y, width, height.
pub type Rect = (f32, f32, f32, f32);

/// The film's moments, in seconds, each a still named for what it shows.
/// The strip's moments, two rows of six, in seconds.
pub const STRIP: [f32; 12] = [0.3, 0.8, 1.15, 1.75, 2.2, 2.45, 2.85, 3.1, 3.58, 4.15, 4.9, 6.6];
/// The joined film's: the before at rest and under the hand, the wipe, then this film's from 2.9 s.
pub const JOINED: [f32; 12] = [0.3, 1.1, 1.6, 2.75, 3.0, 3.25, 3.75, 5.2, 5.9, 6.55, 7.6, 10.05];

pub const STILLS: [(f32, &str); 8] = [
    (0.30, "rest"),
    (1.15, "key-copy"),
    (1.75, "key-paste"),
    (2.45, "rails-awake"),
    (3.10, "field-open"),
    (3.58, "typing"),
    (4.15, "typed"),
    (6.60, "end"),
];

pub struct Interface {
    source: PathBuf,
    /// Whether the hand clicks the number and types; without, it only rests on it.
    clicks: bool,
    path: Track<(f32, f32)>,
    presses: Presses,
    /// The keys struck, each at its second, and how many were sent.
    strokes: Vec<(f32, SharedString)>,
    sent: usize,
}

impl Interface {
    /// The film of `source`, the owner's export.
    pub fn new(source: PathBuf) -> Interface {
        Interface { source, clicks: true, path: Track::default(), presses: Presses::default(), strokes: Vec::new(), sent: 0 }
    }

    /// The same film, the hand resting on the number without a click.
    pub fn untouched(source: PathBuf) -> Interface {
        Interface { clicks: false, ..Interface::new(source) }
    }
}

/// The settings of an export opened fresh: a positive at its rest, nothing moved.
fn opened() -> PipelineSettings {
    PipelineSettings::rest(ConversionMode::Positive, false)
}

/// Every slider's place, readout and value, the rests, the blocks off their rest and the bar's Reset,
/// as the app publishes them for `s`.
fn publish(instance: &ComponentInstance, s: &PipelineSettings) {
    let set = |name: String, value: Value| {
        instance.set_global_property("Store", &name, value).unwrap_or_else(|e| panic!("Store.{name}: {e}"));
    };
    for f in Field::ALL {
        let (v, name) = (f.knob().get(s), f.slint_name());
        let text = Value::String(f.readout(v).into());
        set(format!("{name}-fraction"), Value::Number(f64::from(position_on(f, v, &track_on(f, s)))));
        set(format!("{name}-readout"), text.clone());
        set(format!("{name}-value"), text);
    }
    let shown: Vec<Value> = rests(s).into_iter().map(|r| Value::Number(f64::from(r))).collect();
    set("rests".into(), Value::Model(ModelRc::new(VecModel::from(shown))));
    let mut any = false;
    for (block, name) in [(Block::Color, "color"), (Block::Lighting, "lighting"), (Block::Effects, "effects")] {
        let on = oni_app::store::is_modified(s, block, None);
        any |= on;
        set(format!("{name}-modified"), Value::Bool(on));
    }
    instance.set_property("can-reset", Value::Bool(any)).unwrap_or_else(|e| panic!("can-reset: {e}"));
}

/// The app's part: the readouts read as the app reads them, a slider's steps moving its value on its
/// track, everything shown published again.
fn serve(instance: &ComponentInstance) {
    let fail = |what: &str, e: slint_interpreter::SetCallbackError| panic!("{what}: {e:?}");
    let steps = |args: &[Value]| {
        let text = |i: usize| match args.get(i) {
            Some(Value::String(s)) => s.to_string(),
            _ => String::new(),
        };
        let per = match args.get(2) {
            Some(Value::Number(n)) => *n as f32,
            _ => 0.0,
        };
        Value::Number(f64::from(typed_steps(&text(0), &text(1), per)))
    };
    instance.set_global_callback("Readouts", "steps", steps).unwrap_or_else(|e| fail("Readouts.steps", e));
    let figure = |args: &[Value]| match args.first() {
        Some(Value::String(s)) => Value::String(number(s).into()),
        _ => Value::String(SharedString::default()),
    };
    instance.set_global_callback("Readouts", "number", figure).unwrap_or_else(|e| fail("Readouts.number", e));
    let settings = Rc::new(RefCell::new(opened()));
    let weak = instance.as_weak();
    let step = move |args: &[Value]| {
        let (Some(Value::Number(field)), Some(Value::Number(by))) = (args.first(), args.get(1)) else { return Value::Void };
        let (Some(f), Some(instance)) = (Field::from_index(*field as i32), weak.upgrade()) else { return Value::Void };
        let next = {
            let s = settings.borrow();
            f.knob().set(&s, stepped(f, f.knob().get(&s), *by as i32, &track_on(f, &s)))
        };
        publish(&instance, &next);
        *settings.borrow_mut() = next;
        Value::Void
    };
    instance.set_global_callback("Store", "step", step).unwrap_or_else(|e| fail("Store.step", e));
}

/// Every element shown with this role and label, where the layout put it, in points.
pub fn placed(stage: &Stage, role: AccessibleRole, label: &str) -> Vec<Rect> {
    ElementHandle::find_by_accessible_label(stage.instance(), label)
        .filter(|e| e.accessible_role() == Some(role) && e.size().width > 0.0)
        .map(|e| {
            let (at, size) = (e.absolute_position(), e.size());
            (at.x, at.y, size.width, size.height)
        })
        .collect()
}

fn the(stage: &Stage, role: AccessibleRole, label: &str) -> Result<Rect, String> {
    placed(stage, role, label).into_iter().next().ok_or_else(|| format!("no {role:?} {label} on the scene"))
}

/// Temperature's slider, its header and rail, in points.
pub fn row(stage: &Stage) -> Result<Rect, String> {
    the(stage, AccessibleRole::Slider, oni_i18n::tr("Temperature"))
}

/// Where the pane ends, on a whole row: under Lighting's first slider if the stage holds it whole with
/// its block's margin, else under the line that opens Lighting.
fn foot(stage: &Stage) -> Result<f32, String> {
    let after = stage.number("section-after");
    let first = the(stage, AccessibleRole::Slider, oni_i18n::tr("Exposure"))?;
    if first.1 + first.3 + after <= super::SIZE.1 {
        return Ok(first.1 + first.3 + after);
    }
    let last = the(stage, AccessibleRole::Slider, oni_i18n::tr("Saturation"))?;
    Ok(last.1 + last.3 + after + stage.number("line"))
}

/// The centre of Temperature's number, between the minus and the plus its face opens with under the
/// hand: found by laying the hand on the readout once, before the film.
fn figure(stage: &Stage, slider: Rect) -> Result<(f32, f32), String> {
    let readout = Field::Temperature.readout(Field::Temperature.knob().get(&opened()));
    let shown = placed(stage, AccessibleRole::Text, &readout).into_iter().find(|r| r.1 >= slider.1 && r.1 < slider.1 + slider.3);
    let shown = shown.ok_or("no readout on Temperature's row")?;
    let window = stage.instance().window();
    window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(shown.0 + shown.2 / 2.0, shown.1 + shown.3 / 2.0) });
    slint::platform::update_timers_and_animations();
    let lower = the(stage, AccessibleRole::Button, oni_i18n::tr("One step lower"))?;
    let higher = the(stage, AccessibleRole::Button, oni_i18n::tr("One step higher"))?;
    window.dispatch_event(WindowEvent::PointerExited);
    slint::platform::update_timers_and_animations();
    Ok(((lower.0 + lower.2 + higher.0) / 2.0, lower.1 + lower.3 / 2.0))
}

impl Demo for Interface {
    fn topic(&self) -> &'static str {
        "welcome/interface"
    }

    fn component(&self) -> &'static str {
        "Interface"
    }

    fn seconds(&self) -> f32 {
        SECONDS
    }

    fn size(&self) -> (f32, f32) {
        super::SIZE
    }

    fn source(&self) -> PathBuf {
        self.source.clone()
    }

    fn worded(&self) -> bool {
        true
    }

    fn prepare(&mut self, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let instance = stage.instance();
        instance.window().dispatch_event(WindowEvent::WindowActiveChanged(true));
        serve(instance);
        publish(instance, &opened());
        let size = |w: f32, h: f32| ((w * SCALE).round() as u32, (h * SCALE).round() as u32);
        let strip = plain::window(&self.source, size(stage.number("photo-width"), stage.number("photo-height")), CUT)?;
        stage.set_image("photo", &strip);
        slint::platform::update_timers_and_animations();
        let foot = foot(stage)?;
        stage.set("pane-height", Value::Number(f64::from(foot - stage.number("pane-y"))));
        slint::platform::update_timers_and_animations();

        // The tip low and right in each key, so its glyph shows beside the arrow.
        let key = |name: &str| (stage.number(&format!("{name}-x")) + 10.0, stage.number(&format!("{name}-y")) + 10.0);
        let (copy, paste) = (key("copy"), key("paste"));
        let slider = row(stage)?;
        let number = figure(stage, slider)?;
        // Down between Auto White Balance's keycap and the key beside it, so neither answers a hand passing.
        let balance = the(stage, AccessibleRole::Button, oni_i18n::tr("AUTO WHITE BALANCE"))?;
        let options = the(stage, AccessibleRole::Button, oni_i18n::tr("Show Auto White Balance Strength"))?;
        let gap = ((balance.0 + balance.2 + options.0) / 2.0, balance.1 + balance.3 / 2.0);
        // Down the panel's left margin, over no control, to rest on its ground under the pane's last row.
        let margin = stage.number("photo-x") + stage.number("photo-width") + 4.0;
        let rest = (margin, (foot + super::SIZE.1) / 2.0);
        let above = (copy.0 - 30.0, -30.0);
        let (click, typed_from) = (2.95, 3.2);
        let path = Track::from(above).hold(0.5).to(0.85, copy).hold(1.3).to(1.45, paste).hold(1.9).to(2.35, gap);
        self.path = path.to(2.75, number).hold(4.22).to(4.75, (margin, number.1)).to(5.15, rest);
        if self.clicks {
            self.presses = Presses(vec![(click, click + 0.09)]);
            let keys = TYPED.chars().map(|c| SharedString::from(c.to_string())).chain([SharedString::from(Key::Return)]);
            self.strokes = keys.enumerate().map(|(i, k)| (typed_from + i as f32 * STROKE + if i == TYPED.len() { 0.02 } else { 0.0 }, k)).collect();
        }
        Ok(())
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let window = stage.instance().window();
        while let Some((_, text)) = self.strokes.get(self.sent).filter(|(at, _)| *at <= t).cloned() {
            window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            window.dispatch_event(WindowEvent::KeyReleased { text });
            self.sent += 1;
        }
        Ok(())
    }
}

/// The places of the film's moments in its frames.
pub fn still_frames(frames: usize) -> Vec<(usize, &'static str)> {
    STILLS.iter().map(|(s, name)| (((s * crate::film::FPS as f32) as usize).min(frames.saturating_sub(1)), *name)).collect()
}
