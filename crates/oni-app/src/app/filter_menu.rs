//! The gallery's one filter: a menu in three parts, the medals, the bodies and the lenses of the open folder's
//! photos, one pick a part, All Photos over them; its face says what it keeps.

use super::*;
use crate::MenuLine;
use oni_library::index::gear_key;

/// What a line of the filter's menu does.
#[derive(Clone, Debug, PartialEq)]
enum Pick {
    All,
    Medal(Medal),
    Body(String),
    Lens(String),
    Nothing,
}

/// The menu's lines and what each does, from the filter and the open folder's rows; a body or lens picked
/// stays listed while no photo names it any more, so the filter stays seen and undone in one pick.
fn menu(filter: &library::Filter, rows: &[oni_library::Entry]) -> Vec<(MenuLine, Pick)> {
    let line = |text: String, checked: bool, rule: bool, pick: Pick| {
        (MenuLine { text: text.into(), enabled: pick != Pick::Nothing, checked, rule, ..MenuLine::default() }, pick)
    };
    let mut lines = vec![line(tr("All Photos").into(), !filter.cuts(), false, Pick::All)];
    for (i, medal) in [Medal::Gold, Medal::Silver, Medal::Bronze].into_iter().enumerate() {
        let (mut l, p) = line(library::medal(medal), filter.medal == Some(medal), i == 0, Pick::Medal(medal));
        l.medal = library::medal_index(medal);
        lines.push((l, p));
    }
    for lens in [false, true] {
        let picked = if lens { filter.lens.as_deref() } else { filter.body.as_deref() };
        let mut names = library::gear_names(rows, lens);
        if let Some(p) = picked
            && !names.iter().any(|n| gear_key(n) == gear_key(p))
        {
            names.push(p.to_owned());
        }
        if names.is_empty() {
            continue;
        }
        lines.push(line(if lens { tr("Lens") } else { tr("Camera") }.into(), false, true, Pick::Nothing));
        for name in names {
            let checked = picked.is_some_and(|p| gear_key(p) == gear_key(&name));
            lines.push(line(name.clone(), checked, false, if lens { Pick::Lens(name) } else { Pick::Body(name) }));
        }
    }
    lines
}

/// The face: All Photos, else what the filter keeps, its medal, body and lens one after the other.
fn face(filter: &library::Filter) -> String {
    let parts: Vec<String> = [filter.medal.map(library::medal), filter.body.clone(), filter.lens.clone()].into_iter().flatten().collect();
    if parts.is_empty() { tr("All Photos").into() } else { parts.join(" · ") }
}

impl Ctl {
    pub(super) fn publish_filter(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<crate::Library>();
        let s = self.state.borrow();
        let rows = s.folder.as_ref().map(|f| f.rows.as_slice()).unwrap_or_default();
        let lines: Vec<MenuLine> = menu(&s.filter, rows).into_iter().map(|(l, _)| l).collect();
        lib.set_filter_lines(ModelRc::new(VecModel::from(lines)));
        lib.set_filter_face(face(&s.filter).into());
    }

    /// A line of the filter's menu: All Photos clears it; a line keeps its part, or lets it go when it kept it.
    pub(super) fn filter_line_picked(self: &Rc<Self>, at: i32) {
        {
            let mut s = self.state.borrow_mut();
            let rows = s.folder.as_ref().map(|f| f.rows.clone()).unwrap_or_default();
            let Some((line, pick)) = menu(&s.filter, &rows).into_iter().nth(at.max(0) as usize) else { return };
            let (filter, kept) = (&mut s.filter, !line.checked);
            match pick {
                Pick::All => *filter = library::Filter::default(),
                Pick::Medal(m) => filter.medal = kept.then_some(m),
                Pick::Body(name) => filter.body = kept.then_some(name),
                Pick::Lens(name) => filter.lens = kept.then_some(name),
                Pick::Nothing => return,
            }
        }
        self.lay_archive();
        self.publish_bar();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_library::Entry;
    use oni_settings::{Date, Film};

    fn shot(path: &str, body: &str, lens: &str) -> Entry {
        let film = Film { camera_body: Some(body.into()), lens: Some(lens.into()), ..Film::new("Roll") };
        Entry { film: Some(film), ..Entry::new(path, Date::since_reference_date(800_000_000.0), path) }
    }

    #[test]
    fn the_menu_lists_the_medals_then_the_folder_s_bodies_then_its_lenses_one_checked_a_part() {
        let rows = [shot("/a", "Nikon FM2", "Nikkor 50"), shot("/b", "Leica M6", "Nikkor 50")];
        let filter = library::Filter { medal: Some(Medal::Gold), body: Some("nikon fm2".into()), lens: None };
        let lines = menu(&filter, &rows);
        let words: Vec<(String, bool)> = lines.iter().map(|(l, _)| (l.text.to_string(), l.checked)).collect();
        let checked: Vec<&str> = words.iter().filter(|(_, c)| *c).map(|(w, _)| w.as_str()).collect();
        assert_eq!(words.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>(), ["All Photos", "Gold", "Silver", "Bronze", "Camera", "Leica M6", "Nikon FM2", "Lens", "Nikkor 50"]);
        assert_eq!(checked, ["Gold", "Nikon FM2"], "one line checked a part, a name matched as the counts match it");
        assert!(!lines[4].0.enabled && lines[4].0.rule, "each part's heading a line no hand picks, under a rule");
        assert_eq!(face(&filter), "Gold · nikon fm2");
        assert_eq!(face(&library::Filter::default()), "All Photos", "twin: nothing kept, every photo");
    }
}
