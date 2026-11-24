//! Film Type's four tabs on the narrowest window in every language, on the panel and on Prepare's film
//! step: inside their column, each language's words, as its catalogue spells them and Inter sets them,
//! within the track. A build reading the interface live shows English tabs, measured all the same.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{PanelState, PanelTab};
use oni_i18n::Language;
use slint::ComponentHandle;

use crate::common;

/// The narrowest window the app opens at, `ShellStyle.min-width`.
const NARROWEST: u32 = 1100;
/// `Type.label`, the tabs' words.
const LABEL: f32 = 13.0;
/// `TabsStyle.margin`, the least room either side of a tab's words; `inset` and `gap` about the tabs.
const MARGIN: f32 = 8.0;
const INSET: f32 = 4.0;
const GAP: f32 = 1.0;
const TABS: [&str; 4] = ["Negative", "Positive", "B&W", "Digital"];

/// The width `text` takes at the tabs' size in Inter, the interface's face, at its resting weight.
fn set_width(text: &str) -> f32 {
    let face = swash::FontRef::from_index(include_bytes!("../../../oni-ui/fonts/Inter.ttf"), 0).expect("Inter reads");
    let advances = face.glyph_metrics(&[]).scale(LABEL);
    text.chars().map(|c| advances.advance_width(face.charmap().map(c))).sum()
}

/// The room `tabs`' words need in the language spoken: each its words and a margin either side.
fn needed(tabs: &[&'static str]) -> f32 {
    tabs.iter().map(|t| set_width(oni_i18n::tr(t)) + 2.0 * MARGIN).sum::<f32>() + 2.0 * INSET + (tabs.len() - 1) as f32 * GAP
}

/// Every tab's name the window shows, to tell what a missing tab stood among.
fn tab_names(w: &oni_app::MainWindow) -> Vec<String> {
    use i_slint_backend_testing::ElementRoot;
    let tabs = w.root_element().query_descendants().match_accessible_role(Role::Tab).find_all();
    tabs.into_iter().filter_map(|e| e.accessible_label().map(|l| l.to_string())).collect()
}

pub(crate) fn film_types_tabs_fit_the_narrowest_window_in_every_language() {
    let Some(app) = crate::panel_bindings::opened("film-type-fit") else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let panel = w.global::<PanelState>();
    rig.window.set_size(slint::PhysicalSize::new(NARROWEST, 760));
    let mut needs = Vec::new();
    for language in [Language::French, Language::Spanish, Language::English] {
        oni_app::language::speak(Some(language));
        panel.set_tab(PanelTab::Roll);
        panel.invoke_tab_picked(PanelTab::Roll);
        rig.settle();
        let shot = rig.shot(&format!("film-type-{language:?}"));
        // The tabs as the window lays them, by the words it shows: the catalogue's where compiled.
        let tab = |name: &'static str| {
            let shown = oni_i18n::tr(name);
            let found = common::placed(w, Role::Tab, shown).into_iter().chain(common::placed(w, Role::Tab, name)).next();
            found.unwrap_or_else(|| panic!("{language:?}: no {shown} tab among {:?}", tab_names(w)))
        };
        let tabs: Vec<common::Placed> = TABS.iter().map(|t| tab(t)).collect();
        let (left, right) = (tabs[0].x, tabs[3].x + tabs[3].width);
        let block = (w.get_panel_x(), NARROWEST as f32);
        assert!(tabs.windows(2).all(|p| p[1].x >= p[0].x + p[0].width - 0.5), "{language:?}: tabs in order, none over another: {tabs:?}");
        assert!(left > block.0 && right < block.1, "{language:?}: the tabs inside the panel {block:?}: {left} to {right}");
        let (track, need) = (right - left + 2.0 * INSET, needed(&TABS));
        println!("{language:?}: the words need {need:.1} pt of the track's {track:.1}; shot {}", shot.display());
        assert!(need <= track, "{language:?}: Film Type's words need {need:.1} pt, the track holds {track:.1}");
        needs.push(need);
    }
    // Prepare the Roll's film step holds the same four tabs in its own column, under its field's label.
    let p = w.global::<oni_app::Preparing>();
    shell.ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", std::time::Duration::from_secs(30), || p.get_shown() && p.get_step() == 0);
    for language in [Language::French, Language::Spanish, Language::English] {
        oni_app::language::speak(Some(language));
        rig.settle();
        let tab = |name: &'static str| {
            let shown = oni_i18n::tr(name);
            let found = common::placed(w, Role::Tab, shown).into_iter().chain(common::placed(w, Role::Tab, name)).next();
            found.unwrap_or_else(|| panic!("{language:?}: no {shown} tab on Prepare's film step among {:?}", tab_names(w)))
        };
        let tabs: Vec<common::Placed> = TABS.iter().map(|t| tab(t)).collect();
        let (left, right) = (tabs[0].x, tabs[3].x + tabs[3].width);
        assert!(tabs.windows(2).all(|p| p[1].x >= p[0].x + p[0].width - 0.5), "{language:?}: Prepare's tabs in order, none over another: {tabs:?}");
        assert!(left > 0.0 && right < NARROWEST as f32, "{language:?}: Prepare's tabs inside the window: {left} to {right}");
        let (track, need) = (right - left + 2.0 * INSET, needed(&TABS));
        println!("{language:?}: Prepare's film tabs need {need:.1} pt of the track's {track:.1}; shot {}", rig.shot(&format!("prepare-film-type-{language:?}")).display());
        assert!(need <= track, "{language:?}: Prepare's film tabs need {need:.1} pt, the track holds {track:.1}");
        // Twin: the three film tabs alone measure less than the four the step lays.
        assert!(needed(&TABS[..3]) < need, "{language:?}: Digital's words counted");
    }
    oni_app::language::speak(Some(Language::English));
    rig.settle();
    // Twin: the words measured are each catalogue's own, French's Numérique and Spanish's Negativo, not English's.
    assert!(needs[0] != needs[2] && needs[1] != needs[2], "{needs:?}");
}
