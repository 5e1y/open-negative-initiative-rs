//! The bar on the narrowest screen it must fit, every word whole in every language, its ends on the columns'.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Bar;
use slint::ComponentHandle;

use crate::common;

/// A 13-inch MacBook Air's screen, in points, the narrowest the bar must hold every word on.
const AIR_13: f32 = 1440.0;
/// The room the Mac's traffic lights take at the bar's start, as `chrome::traffic_lights` lays them.
const LIGHTS: f32 = 94.0;

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
    let _ = std::fs::remove_dir_all(&root);
}
