//! Every slider carries a − and a + key at its rail's ends: a press steps three of its value editor's
//! steps, renders and records one undo step, and each key's tooltip names the slider, the way and the step.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle, ElementRoot};
use oni_app::{PanelState, PanelTab};
use oni_i18n::Language;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::time::Duration;

use crate::common::{self, Placed};
use crate::film_effects::{opened, photo};

const MINUS: char = '\u{2212}';
/// `SliderStyle.row`, a knob's height.
const KNOB_ROW: f32 = 26.0;

/// Every slider shown: its name, its value as assistive tools read it, and where the layout put it. A level's
/// handle, one of the five on a plot's shared rail, reads as a slider too but is a knob alone, a row tall,
/// with no readout or value editor to step by: the sliders are the ones with their readout above the row.
fn sliders(w: &oni_app::MainWindow) -> Vec<(String, String, Placed)> {
    w.root_element()
        .query_descendants()
        .match_accessible_role(Role::Slider)
        .find_all()
        .into_iter()
        .map(|e| {
            let (at, size) = (e.absolute_position(), e.size());
            let name = e.accessible_label().unwrap_or_default().to_string();
            (name, e.accessible_value().unwrap_or_default().to_string(), Placed { x: at.x, y: at.y, width: size.width, height: size.height })
        })
        .filter(|(_, _, s)| s.height > KNOB_ROW + 1.0)
        .collect()
}

/// The keys named for `name` on the row `s` lays: its − key at the left end of its foot, its + key at the right.
fn keys_on(w: &oni_app::MainWindow, name: &str, s: &Placed) -> (usize, usize) {
    // Centred on the rail's row, a dial's a few points taller than the key.
    let on = |k: &Placed| (k.y + k.height - s.y - s.height).abs() <= 3.0 && k.x >= s.x - 0.5 && k.x + k.width <= s.x + s.width + 0.5;
    let minus = common::keys_named(w, &format!("{name} {MINUS}:")).into_iter().filter(|k| on(k) && (k.x - s.x).abs() < 1.0).count();
    let plus = common::keys_named(w, &format!("{name} +:")).into_iter().filter(|k| on(k) && (k.x + k.width - s.x - s.width).abs() < 1.0).count();
    (minus, plus)
}

/// The sliders shown, each keyed on its own row, and every key shown standing on one: their count.
fn keyed(w: &oni_app::MainWindow, panel: &str) -> usize {
    let shown = sliders(w);
    for (name, _, s) in &shown {
        assert_eq!(keys_on(w, name, s), (1, 1), "{panel}: {name}'s − and + keys at its rail's ends: {s:?} {:?}", common::keys_named(w, name));
    }
    // Counted against the sliders: no key stands apart from its slider, and none is missing.
    let all = |sign: char| signed_keys(w, sign);
    assert_eq!((all(MINUS), all('+')), (shown.len(), shown.len()), "{panel}: one − and one + key a slider");
    // Twin: the row under a slider holds none of its keys, so the count binds each key to its own slider.
    if let Some((name, _, s)) = shown.first() {
        assert_eq!(keys_on(w, name, &Placed { y: s.y + s.height, ..*s }), (0, 0), "{panel}: {name}'s keys found off its row");
    }
    println!("{panel}: {} sliders, each with its two keys", shown.len());
    shown.len()
}

/// The keys shown whose tooltip names a slider and `sign`'s way, as "Exposure −: lower by 0.03 EV".
fn signed_keys(w: &oni_app::MainWindow, sign: char) -> usize {
    let words = format!(" {sign}: ");
    w.root_element()
        .query_descendants()
        .match_accessible_role(Role::Button)
        .find_all()
        .into_iter()
        .filter(|e| e.accessible_label().is_some_and(|l| l.contains(&words)))
        .count()
}

/// The value the slider so named shows, as assistive tools read it.
fn value(w: &oni_app::MainWindow, name: &str) -> String {
    let mut found = ElementHandle::find_by_accessible_label(w, name).filter(|e| e.accessible_role() == Some(Role::Slider));
    found.next().unwrap_or_else(|| panic!("a {name} slider shown")).accessible_value().unwrap_or_default().to_string()
}

fn number(text: &str) -> (f64, u32) {
    oni_app::slider_track::read(text.rsplit('\u{2026}').next().unwrap_or(text)).unwrap_or_else(|| panic!("a number in {text:?}"))
}

/// A press on the slider so named's − or + key: the shown units its value moved, one of its readout's last digit.
fn pressed(rig: &common::Rig, w: &oni_app::MainWindow, name: &str, plus: bool) -> f64 {
    let before = number(&value(w, name));
    let key = common::slider_key(w, name, plus);
    common::click(rig, w, key.middle());
    rig.settle();
    let after = number(&value(w, name));
    ((after.0 - before.0) / 10f64.powi(-(before.1 as i32))).round()
}

/// A press on the value editor's own +, which opens under the hand on the slider's readout.
fn edited(rig: &common::Rig, w: &oni_app::MainWindow, name: &str) -> f64 {
    let before = number(&value(w, name));
    let s = common::the(w, Role::Slider, name);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(s.x + s.width - 50.0, s.y + 8.0) });
    rig.settle();
    let higher = common::the(w, Role::Button, "One step higher");
    common::click(rig, w, higher.middle());
    rig.settle();
    let after = number(&value(w, name));
    ((after.0 - before.0) / 10f64.powi(-(before.1 as i32))).round()
}

/// A press three of the value editor's steps either way, and the value editor's own one: the readout's
/// shown units.
fn steps_three(rig: &common::Rig, w: &oni_app::MainWindow, name: &str) {
    assert_eq!(pressed(rig, w, name, true), 3.0, "{name}'s + key moves three shown units");
    assert_eq!(pressed(rig, w, name, false), -3.0, "{name}'s − key moves three shown units back");
    // Twin: the value editor's own + moves one, a third of the key's press.
    assert_eq!(edited(rig, w, name), 1.0, "{name}'s value editor steps one shown unit");
}

/// Every block of the panel unfolded and every settings key open, so a tab lays all its sliders.
fn unfolded(rig: &common::Rig, w: &oni_app::MainWindow) {
    for _ in 0..20 {
        let shut: Vec<ElementHandle> = w
            .root_element()
            .query_descendants()
            .match_accessible_role(Role::Button)
            .find_all()
            .into_iter()
            .filter(|e| e.absolute_position().x >= w.get_panel_x() && e.accessible_label().is_some_and(|l| l.starts_with("Show ")))
            .collect();
        if shut.is_empty() {
            return;
        }
        for e in shut {
            e.invoke_accessible_default_action();
        }
        rig.settle();
    }
}

fn tab(rig: &common::Rig, w: &oni_app::MainWindow, t: PanelTab) {
    w.global::<PanelState>().set_tab(t);
    w.global::<PanelState>().invoke_tab_picked(t);
    rig.settle();
}

/// The panel scrolled down until the slider so named stands in the window's lower half, or as far as it goes.
fn scrolled_to(rig: &common::Rig, w: &oni_app::MainWindow, name: &str) {
    let height = rig.window.size().height as f32 / rig.window.scale_factor();
    let at = LogicalPosition::new(w.get_panel_x() + 160.0, height * 0.5);
    for _ in 0..40 {
        // A slider scrolled out of the window is out of the tree too.
        if common::placed(w, Role::Slider, name).first().is_some_and(|s| s.y < height * 0.5) {
            break;
        }
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
        rig.window.dispatch_event(WindowEvent::PointerScrolled { position: at, delta_x: 0.0, delta_y: -120.0 });
        rig.pump();
    }
    rig.settle();
}

/// Each panel drawn at a 14-inch window, 1512 by 982, and at the narrowest window the shell takes, its
/// panel's width the same: the shots a person judges the keys by.
fn shots(rig: &common::Rig, w: &oni_app::MainWindow) {
    let narrowest = (w.global::<oni_app::DS>().get_control_min_window_width() as u32, w.global::<oni_app::DS>().get_control_min_window_height() as u32);
    for (size, (width, height)) in [("1512x982", (1512, 982)), ("narrowest", narrowest)] {
        rig.window.set_size(slint::PhysicalSize::new(2 * width, 2 * height));
        rig.settle();
        tab(rig, w, PanelTab::Color);
        println!("shot {}", rig.shot(&format!("slider-keys-colour-{size}")).display());
        scrolled_to(rig, w, "Exposure");
        println!("shot {}", rig.shot(&format!("slider-keys-lighting-{size}")).display());
        scrolled_to(rig, w, "Vignette");
        println!("shot {}", rig.shot(&format!("slider-keys-effects-{size}")).display());
        for (t, panel) in [(PanelTab::Details, "details"), (PanelTab::Roll, "roll"), (PanelTab::Crop, "crop"), (PanelTab::Dedust, "dedust")] {
            tab(rig, w, t);
            println!("shot {}", rig.shot(&format!("slider-keys-{panel}-{size}")).display());
        }
        w.global::<oni_app::App>().invoke_menu(oni_app::Act::Settings);
        rig.settle();
        println!("shot {}", rig.shot(&format!("slider-keys-settings-{size}")).display());
        rig.key("\u{1b}");
        rig.settle();
    }
}

pub(crate) fn every_slider_has_its_keys_and_a_press_is_three_shown_units() {
    let root = common::Root::new("slider-keys");
    photo(&root.join("Scans"));
    let Some(app) = opened(root) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    shots(rig, w);
    // Tall enough for every block of a tab unscrolled.
    rig.window.set_size(slint::PhysicalSize::new(2 * 1512, 2 * 4000));
    rig.settle();

    // Each tab's sliders, every one keyed on its row: the digital photo's, then a negative's Roll tab.
    let mut counts = Vec::new();
    for (t, panel) in [(PanelTab::Color, "Colour"), (PanelTab::Details, "Details"), (PanelTab::Roll, "Roll"), (PanelTab::Crop, "Crop"), (PanelTab::Dedust, "De-dust")] {
        tab(rig, w, t);
        unfolded(rig, w);
        counts.push((panel, keyed(w, panel)));
    }
    assert!(counts.iter().all(|(_, n)| *n > 0), "every tab holds sliders: {counts:?}");
    // Twin: the Roll tab's level handles, read as sliders but keyless, would fail the count.
    tab(rig, w, PanelTab::Roll);
    let handles: Vec<(String, Placed)> = w
        .root_element()
        .query_descendants()
        .match_accessible_role(Role::Slider)
        .find_all()
        .into_iter()
        .filter(|e| e.size().height <= KNOB_ROW + 1.0)
        .map(|e| (e.accessible_label().unwrap_or_default().to_string(), Placed { x: e.absolute_position().x, y: e.absolute_position().y, width: e.size().width, height: e.size().height }))
        .collect();
    assert!(!handles.is_empty() && handles.iter().all(|(name, h)| keys_on(w, name, h) == (0, 0)), "keyless handles: {handles:?}");

    // Lighting's Exposure: a press renders and is one step of the history, which one undo takes back.
    tab(rig, w, PanelTab::Color);
    let settings = || ctl.state.borrow().store().settings().clone();
    let (exposure, renders) = (value(w, "Exposure"), ctl.state.borrow().renders());
    let plus = common::slider_key(w, "Exposure", true);
    common::click(rig, w, plus.middle());
    rig.until("the keyed render", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert_ne!(value(w, "Exposure"), exposure, "the press moved Exposure");
    assert!(ctl.state.borrow().store().can_undo());
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(value(w, "Exposure"), exposure, "one undo takes the press back");
    assert_eq!(settings().mode, oni_settings::ConversionMode::Digital, "and the press alone: the film type stays");
    // Twin: a second undo reaches the step before, the film type picked.
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_ne!(settings().mode, oni_settings::ConversionMode::Digital, "the step before the press is the film type's");
    ctl.act(oni_app::Act::Redo);
    rig.settle();
    assert_eq!(settings().mode, oni_settings::ConversionMode::Digital);

    // One slider of each block and tab: Lighting, Colour, Effects, Film Effects, Details, Roll, Crop's dial,
    // De-dust's brush.
    for name in ["Exposure", "Tint", "Vignette"] {
        steps_three(rig, w, name);
    }
    // A film effect walks its named levels: each key, and the value editor's own step, one notch.
    for (name, rest, up) in [("Grain", "Medium", "Strong"), ("Grain size", "Medium", "Coarse")] {
        assert!(value(w, name).ends_with(rest), "{name} at rest: {}", value(w, name));
        common::click(rig, w, common::slider_key(w, name, true).middle());
        rig.settle();
        assert!(value(w, name).ends_with(up), "{name}'s + key, one notch: {}", value(w, name));
        common::click(rig, w, common::slider_key(w, name, false).middle());
        rig.settle();
        assert!(value(w, name).ends_with(rest), "{name}'s − key, one notch back: {}", value(w, name));
        // Twin: the value editor's own + reaches the same notch, never a digit of the readout.
        assert!(edited(rig, w, name).abs() > 1.0, "{name}'s value editor steps a notch");
        assert!(value(w, name).ends_with(up), "{}", value(w, name));
    }
    for (t, name) in [(PanelTab::Details, "Texture"), (PanelTab::Roll, "Distortion"), (PanelTab::Crop, "Angle"), (PanelTab::Dedust, "Brush size")] {
        tab(rig, w, t);
        steps_three(rig, w, name);
    }

    // The keys' tooltips are the catalogue's texts, so they speak the app's language: the interface read
    // at run time stays English, the compiled one shows the French and Spanish laid for each.
    tab(rig, w, PanelTab::Color);
    let pattern = format!("{{}} {MINUS}: lower by {{}}");
    let english_words = pattern.replacen("{}", "Exposure", 1).replacen("{}", "0.03 EV", 1);
    assert_eq!(common::placed(w, Role::Button, &english_words).len(), 1, "the − key names Exposure, its way and its press");
    for language in [Language::French, Language::Spanish] {
        let words = oni_i18n::translated(language, &pattern, None).unwrap_or_else(|| panic!("{pattern:?} in {language:?}"));
        assert!(words != pattern && words.matches("{}").count() == 2, "{language:?}: {words}");
    }
    // Twin: a tooltip written outside the catalogue has no translation to show.
    assert!(oni_i18n::translated(Language::French, &english_words, None).is_none());

    // Colour read as C, M and Y: a press three CC, the value editor's one.
    ctl.act(oni_app::Act::FiltrationCmy);
    rig.settle();
    unfolded(rig, w);
    counts.push(("Colour as C, M and Y", keyed(w, "Colour as C, M and Y")));
    steps_three(rig, w, "C");
    ctl.act(oni_app::Act::FiltrationCmy);
    rig.settle();

    // A negative's Roll tab, its density and dyes.
    let n = ctl.state.borrow().renders();
    w.global::<PanelState>().invoke_film_type_picked(0);
    rig.until("the negative rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > n);
    tab(rig, w, PanelTab::Roll);
    unfolded(rig, w);
    counts.push(("Roll (negative)", keyed(w, "Roll (negative)")));
    steps_three(rig, w, "Ceiling");

    // The export's Quality.
    ctl.act(oni_app::Act::Export);
    rig.settle();
    unfolded(rig, w);
    counts.push(("Export", keyed(w, "Export")));
    steps_three(rig, w, "Quality");
    println!("sliders keyed: {counts:?}");
}
