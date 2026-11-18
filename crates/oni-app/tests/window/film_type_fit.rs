//! Film Type's four tabs on the narrowest window, in every language the app speaks: the tabs inside the
//! panel's block, and each language's words, as its catalogue spells them and Inter sets them, within the
//! track. A build reading the interface live shows English tabs; its words are measured all the same.

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

/// The room the four tabs' words need in the language spoken: each its words and a margin either side.
fn needed() -> f32 {
    TABS.iter().map(|t| set_width(oni_i18n::tr(t)) + 2.0 * MARGIN).sum::<f32>() + 2.0 * INSET + 3.0 * GAP
}

/// Every tab's name the window shows, to tell what a missing tab stood among.
fn tab_names(w: &oni_app::MainWindow) -> Vec<String> {
    use i_slint_backend_testing::ElementRoot;
    let tabs = w.root_element().query_descendants().match_accessible_role(Role::Tab).find_all();
    tabs.into_iter().filter_map(|e| e.accessible_label().map(|l| l.to_string())).collect()
}

pub(crate) fn film_types_tabs_fit_the_narrowest_window_in_every_language() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("film-type-fit") else { return };
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
        let (track, need) = (right - left + 2.0 * INSET, needed());
        println!("{language:?}: the words need {need:.1} pt of the track's {track:.1}; shot {}", shot.display());
        assert!(need <= track, "{language:?}: Film Type's words need {need:.1} pt, the track holds {track:.1}");
        needs.push(need);
    }
    oni_app::language::speak(Some(Language::English));
    rig.settle();
    // Twin: the words measured are each catalogue's own, French's Numérique and Spanish's Negativo, not English's.
    assert!(needs[0] != needs[2] && needs[1] != needs[2], "{needs:?}");
    let _ = std::fs::remove_dir_all(&root);
}
