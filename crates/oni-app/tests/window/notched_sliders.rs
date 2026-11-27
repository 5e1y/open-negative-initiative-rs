//! A digital photo's notched slider under the hand: a slow drag lays the knob on each notch it crosses, never between,
//! its readout naming the notch in the same turn; a − or + key and an arrow move exactly one notch, the knob there at once.

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementRoot};
use oni_app::Store;
use oni_app::fields::Field;
use oni_settings::effect_levels::GRAIN_AMOUNT;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::time::{Duration, Instant};

use crate::common::{self, Placed};
use crate::film_effects::{opened, photo};

/// The centre the knob of the slider laid at `slider` is drawn at, as a share of its rail's travel.
fn drawn(w: &oni_app::MainWindow, slider: &Placed, rail: &Placed) -> f32 {
    let knobs: Vec<Placed> = w
        .root_element()
        .query_descendants()
        .match_inherits("Knob")
        .find_all()
        .into_iter()
        .map(|e| Placed { x: e.absolute_position().x, y: e.absolute_position().y, width: e.size().width, height: e.size().height })
        .filter(|k| k.y >= slider.y && k.y + k.height <= slider.y + slider.height + 0.5)
        .collect();
    assert_eq!(knobs.len(), 1, "one knob on the slider at {slider:?}: {knobs:?}");
    let knob = knobs[0];
    // The knob travels between the rail's ends less half its width either side.
    (knob.x + knob.width / 2.0 - rail.x - knob.width / 2.0) / (rail.width - knob.width)
}

/// Whether `at` lies on one of `n` notches laid evenly along the travel.
fn on_notch(at: f32, n: usize) -> bool {
    let k = at * (n - 1) as f32;
    (k - k.round()).abs() < 2e-3
}

/// A slow drag along the rail of `label` from one end to the other, `steps` moves: the store's fraction and the drawn
/// knob after each, with the readout shown.
fn dragged(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, f: Field, steps: usize) -> Vec<(f32, f32, String)> {
    let g = w.global::<Store>();
    let (slider, rail) = (common::the(w, AccessibleRole::Slider, label), common::rail(w, label));
    let y = rail.y + rail.height - 13.0;
    let x = |share: f32| rail.x + 10.0 + share * (rail.width - 20.0);
    let start = LogicalPosition::new(x(0.0), y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: start });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: start, button: PointerEventButton::Left });
    rig.pump();
    let mut seen = vec![];
    for i in 1..=steps {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x(i as f32 / steps as f32), y) });
        rig.pump();
        seen.push((oni_app::app::fraction(&g, f), drawn(w, &slider, &rail), readout(&g, f)));
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(x(1.0), y), button: PointerEventButton::Left });
    rig.settle();
    seen
}

fn readout(g: &Store, f: Field) -> String {
    match f {
        Field::GrainAmount => g.get_grain_amount_readout().into(),
        Field::Exposure => g.get_exposure_readout().into(),
        _ => unreachable!("a field this check drags"),
    }
}

/// How long after `act` has run the knob drawn and the store's fraction both stand on `wanted`, pumping as a frame
/// would: zero when they stand there in the turn that took the press.
fn landed_after(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, f: Field, wanted: f32, act: impl FnOnce()) -> Duration {
    let g = w.global::<Store>();
    let (slider, rail) = (common::the(w, AccessibleRole::Slider, label), common::rail(w, label));
    act();
    let asked = Instant::now();
    let off = || (oni_app::app::fraction(&g, f) - wanted).abs() > 2e-3 || (drawn(w, &slider, &rail) - wanted).abs() > 2e-3;
    if !off() {
        return Duration::ZERO;
    }
    while off() {
        assert!(asked.elapsed() < Duration::from_secs(3), "{label} never landed on {wanted}: drawn {}", drawn(w, &slider, &rail));
        std::thread::sleep(Duration::from_millis(1));
        rig.pump();
    }
    asked.elapsed()
}

pub(crate) fn a_notched_slider_lands_on_each_notch_under_the_hand_and_each_key_moves_one_at_once() {
    let root = common::Root::new("notched");
    photo(&root.join("Scans"));
    let Some(app) = opened(root) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let n = GRAIN_AMOUNT.len();
    let grain = || ctl.state.borrow().store().settings().grain.amount;

    // The − and + keys and the arrows: one notch each, the knob on it at once, the store with it.
    let at = |i: usize| i as f32 / (n - 1) as f32;
    let g = w.global::<Store>();
    let rest = (oni_app::app::fraction(&g, Field::GrainAmount) * (n - 1) as f32).round() as usize;
    let slider = || ElementHandle::find_by_accessible_label(w, "Grain").find(|e| e.accessible_role() == Some(AccessibleRole::Slider)).expect("Grain");
    let minus = common::slider_key(w, "Grain", false);
    let plus = common::slider_key(w, "Grain", true);
    let mut took = vec![];
    for (what, by) in [("−", -1), ("+", 1), ("←", -1), ("←", -1), ("→", 1)] {
        let from = (oni_app::app::fraction(&g, Field::GrainAmount) * (n - 1) as f32).round() as i32;
        let notch = (from + by) as usize;
        let renders = ctl.state.borrow().renders();
        let t = landed_after(rig, w, "Grain", Field::GrainAmount, at(notch), || match what {
            "−" => common::click(rig, w, minus.middle()),
            "+" => common::click(rig, w, plus.middle()),
            "←" => slider().invoke_accessible_decrement_action(),
            _ => slider().invoke_accessible_increment_action(),
        });
        let asked = Instant::now() - t;
        while ctl.state.borrow().renders() <= renders && asked.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(1));
            rig.pump();
        }
        let image = asked.elapsed().as_secs_f64() * 1000.0;
        println!("Grain {what}: the knob on its notch {:.1} ms after the press, the image {image:.0} ms", t.as_secs_f64() * 1000.0);
        assert_eq!(grain(), GRAIN_AMOUNT[notch], "{what} moves one notch");
        took.push((what, t));
    }
    assert!(rest >= 2, "Grain rests two notches up at least");
    assert!(took.iter().all(|(_, t)| t.is_zero()), "the knob waited after a key: {took:?}");
    rig.settle();

    // A slow drag: every move lays the knob on a notch, the drawn one with it, and the readout names it in the same turn.
    let seen = dragged(rig, w, "Grain", Field::GrainAmount, 60);
    for (stored, shown, said) in &seen {
        assert!(on_notch(*stored, n) && (shown - stored).abs() < 2e-3, "the knob between notches under the hand: stored {stored}, drawn {shown}, {said}");
    }
    let crossed: Vec<f32> = seen.iter().map(|s| s.0).fold(vec![], |mut v, at| {
        if v.last() != Some(&at) {
            v.push(at);
        }
        v
    });
    assert_eq!(crossed.len(), n, "each notch crossed in turn: {crossed:?}");
    let named: Vec<&String> = seen.iter().map(|s| &s.2).fold(vec![], |mut v, said| {
        if v.last() != Some(&said) {
            v.push(said);
        }
        v
    });
    assert_eq!(named.len(), n, "the readout turns with the knob, never apart: {named:?}");
    assert_eq!(grain(), GRAIN_AMOUNT[n - 1], "the drag lays the notch it ended on");
    // Twin: a continuous slider's knob floats under the same drag, between any two values.
    let exposure = dragged(rig, w, "Exposure", Field::Exposure, 60);
    assert!(exposure.iter().any(|s| !on_notch(s.0, n)), "Exposure dragged, as notched as Grain: {:?}", exposure.iter().map(|s| s.0).collect::<Vec<_>>());

    rig.settle();
    // Twin: Exposure's − still springs its knob over the settle, a value moved by a continuous step.
    let (slider, rail) = (common::the(w, AccessibleRole::Slider, "Exposure"), common::rail(w, "Exposure"));
    common::click(rig, w, common::slider_key(w, "Exposure", false).middle());
    let pressed = drawn(w, &slider, &rail);
    rig.settle();
    let rested = drawn(w, &slider, &rail);
    assert!((pressed - rested).abs() > 1e-3, "a continuous slider's step springs: {pressed} at the press, {rested} at rest");
}
