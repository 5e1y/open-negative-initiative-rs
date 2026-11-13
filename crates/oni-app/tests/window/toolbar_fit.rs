//! The bar on the narrowest screen it must fit, every word whole in every language, its ends on the columns';
//! on the narrowest window, its glyphs alone on square keys, every key inside it.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Bar;
use slint::ComponentHandle;

use crate::common;

/// A 13-inch MacBook Air's screen, in points, the narrowest the bar must hold every word on.
const AIR_13: f32 = 1440.0;
/// The room the Mac's traffic lights take at the bar's start, as `chrome::traffic_lights` lays them on
/// macOS 26 (read off the app's log after a full screen's round trip).
const LIGHTS: f32 = 100.0;
/// The narrowest window the app opens at, `ShellStyle.min-width`.
const NARROWEST: f32 = 1100.0;
/// `ShellStyle.bar-end`.
const END: f32 = 16.0;

pub(crate) fn the_bar_holds_every_word_on_a_13_inch_screen() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("bar-13") else { return };
    let w = &shell.window;
    let bar = w.global::<Bar>();
    bar.set_traffic_lights(LIGHTS);
    let worded = |width: f32| {
        rig.window.set_size(slint::PhysicalSize::new(width as u32, 900));
        rig.settle();
        !common::placed(w, Role::Button, "Undo").is_empty()
    };
    // The editor's bar, the fullest, in every language the app speaks, shot on the screen it must fit.
    let needs: Vec<(oni_i18n::Language, f32)> = [oni_i18n::Language::English, oni_i18n::Language::French, oni_i18n::Language::Spanish]
        .into_iter()
        .map(|language| {
            oni_app::language::speak(Some(language));
            rig.window.set_size(slint::PhysicalSize::new(AIR_13 as u32, 900));
            rig.settle();
            println!("shot {}", rig.shot(&format!("bar-13-inch-{language:?}")).display());
            (language, bar.get_needed())
        })
        .collect();
    println!("the bar needs {needs:?}");
    assert!(needs.iter().all(|(_, needed)| *needed <= AIR_13), "over a 13-inch screen's {AIR_13} pt: {needs:?}");
    oni_app::language::speak(Some(oni_i18n::Language::English));
    rig.settle();
    // The width read is the one the words give way under: whole at it; twin: a point narrower, glyphs alone.
    let needed = bar.get_needed().ceil();
    assert!(worded(needed), "every word at {needed} pt");
    assert!(!worded(needed - 1.0), "twin: the words give way a point narrower");
    // The bar's ends line up with the columns' content: its first key starts where their leftmost control
    // does, its last ends where their rightmost does; with the traffic lights it starts at their room.
    rig.window.set_size(slint::PhysicalSize::new(AIR_13 as u32, 900));
    let ends = |lights: f32| {
        bar.set_traffic_lights(lights);
        rig.settle();
        let (first, last) = (common::the(w, Role::Button, "BACK"), common::the(w, Role::Button, "EXPORT"));
        let under: Vec<_> = common::placed_all(w, Role::Button).into_iter().filter(|b| b.y > first.y + first.height).collect();
        let left = under.iter().map(|b| b.x).fold(f32::MAX, f32::min);
        let right = under.iter().map(|b| b.x + b.width).fold(0.0, f32::max);
        (first.x - left, right - (last.x + last.width), first.x)
    };
    let (start, end, _) = ends(0.0);
    assert!(start.abs() < 0.5 && end.abs() < 0.5, "the bar's ends on the columns' content: {start} pt and {end} pt off");
    let (_, end, first) = ends(LIGHTS);
    assert!((first - LIGHTS).abs() < 0.5 && end.abs() < 0.5, "twin: with the traffic lights the bar starts at their room: {first} pt, its end {end} pt off");
    // The bar's keys left to right, as wide as tall but for the keycaps and the faces' tabs.
    let keys = |width: f32| {
        rig.window.set_size(slint::PhysicalSize::new(width as u32, 900));
        rig.settle();
        let mut keys: Vec<_> = common::placed_all(w, Role::Button).into_iter().filter(|k| k.y < bar.get_height()).collect();
        keys.sort_by(|a, b| a.x.total_cmp(&b.x));
        keys.dedup_by(|a, b| (a.x - b.x).abs() < 0.5);
        keys
    };
    // On the narrowest window, with the lights, each glyph alone on a square key and every key inside it,
    // none over another; twin: worded, a flat key is wider than tall.
    let narrow = keys(NARROWEST);
    let flat: Vec<_> = narrow.iter().filter(|k| k.width < k.height + 8.0).collect();
    assert!(flat.len() >= 10 && flat.iter().all(|k| (k.width - k.height).abs() < 0.5), "the glyphs alone on square keys: {flat:?}");
    assert!(narrow.last().is_some_and(|k| k.x + k.width <= NARROWEST - END + 0.5), "every key inside the narrowest window: {narrow:?}");
    assert!(narrow.windows(2).all(|p| p[1].x >= p[0].x + p[0].width - 0.5), "no key over another: {narrow:?}");
    println!("shot {}", rig.shot("bar-narrowest").display());
    let wide = keys(AIR_13);
    assert!(wide.iter().any(|k| k.width > k.height + 8.0 && k.height < 49.0 && (k.width - wide[wide.len() - 1].width).abs() > 0.5), "twin: worded, a flat key wider than tall: {wide:?}");
    let _ = std::fs::remove_dir_all(&root);
}

/// The bar's keys left of `bar_height` as laid at `width`, their right end.
fn right_end(rig: &common::Rig, w: &oni_app::MainWindow, width: f32, bar_height: f32) -> f32 {
    rig.window.set_size(slint::PhysicalSize::new(width as u32, 680));
    rig.settle();
    common::placed_all(w, Role::Button).into_iter().filter(|k| k.y < bar_height).map(|k| k.x + k.width).fold(0.0, f32::max)
}

/// On the narrowest window, in every language, the editor's bar and the gallery's end inside the window
/// by the bar's end; the gallery's keycaps keep their words down to the width the bar reads, and a point
/// narrower, where words would run past the edge, wear their glyphs alone.
pub(crate) fn the_bars_last_key_stays_inside_the_narrowest_window_in_every_language() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("bar-narrowest") else { return };
    let w = &shell.window;
    let bar = w.global::<Bar>();
    bar.set_traffic_lights(LIGHTS);
    let height = bar.get_height();
    for language in [oni_i18n::Language::English, oni_i18n::Language::French, oni_i18n::Language::Spanish] {
        oni_app::language::speak(Some(language));
        rig.settle();
        if common::placed(w, Role::Button, oni_i18n::tr("EXPORT")).is_empty() {
            println!("{language:?}: the live interface reads English, judged on the app profile");
            continue;
        }
        let end = right_end(&rig, w, NARROWEST, height);
        assert!(end <= NARROWEST - END + 0.5, "{language:?}, the editor: the last key ends at {end} pt");
        rig.chord(slint::platform::Key::Control, "g");
        let end = right_end(&rig, w, NARROWEST, height);
        println!("shot {}", rig.shot(&format!("bar-narrowest-gallery-{}", language.key())).display());
        assert!(end <= NARROWEST - END + 0.5, "{language:?}, the gallery: the last key ends at {end} pt");
        let worded = || !common::placed(w, Role::Button, oni_i18n::tr("PREPARE ROLL")).is_empty();
        let least = bar.get_least().ceil();
        println!("{language:?}: the gallery's keycaps keep their words from {least} pt");
        let end = right_end(&rig, w, least, height);
        assert!(worded() && end <= least - END + 0.5, "{language:?}: worded at {least} pt, the last key ends at {end} pt");
        right_end(&rig, w, least - 1.0, height);
        assert!(!worded(), "twin, {language:?}: a point narrower the words would pass the edge, and give way");
        if language == oni_i18n::Language::French {
            assert!(least > NARROWEST, "twin: French words cannot fit the narrowest window, {least} pt");
        }
        rig.chord(slint::platform::Key::Control, "g");
    }
    oni_app::language::speak(Some(oni_i18n::Language::English));
    let _ = std::fs::remove_dir_all(&root);
}
