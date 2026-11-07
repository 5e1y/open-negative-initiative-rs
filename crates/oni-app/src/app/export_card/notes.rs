//! The colour space's words, what each space is for and where the photos were edited, each with every
//! text it may say, which its room is fitted to: a preset or a tab changes words, never moves a row.

use super::*;
use oni_i18n::tr;

/// What each space is for, the four built-in ones in their tabs' order then a profile, under the name
/// a lab or a printer will ask for.
fn purposes() -> [&'static str; 5] {
    [
        tr("Web and social networks: what every screen and app reads."),
        tr("Recent phones, tablets and Apple screens: richer reds and greens."),
        tr("Print labs and photo printers: richer cyans and greens."),
        tr("Home printing through the printer's own profile: holds nearly every colour. TIFF 16-bit only."),
        tr("An ICC profile for a paper or a lab's printer."),
    ]
}

/// The purpose in a few words, for the canvas chip; a profile's own name already says it.
fn hints() -> [&'static str; 5] {
    [tr("web & social"), tr("phones & screens"), tr("print"), tr("wide print"), tr("paper")]
}

/// A space's place in `purposes` and `hints`.
fn at(space: &Space) -> usize {
    Space::BUILT_IN.iter().position(|s| s == space).unwrap_or(Space::BUILT_IN.len())
}

pub fn purpose(space: &Space) -> &'static str {
    purposes()[at(space)]
}

pub fn hint(space: &Space) -> &'static str {
    hints()[at(space)]
}

/// What stops a profile: it can no longer be read, or none is installed to choose.
fn troubles() -> [&'static str; 2] {
    [
        tr("This profile can no longer be read. Choose another one."),
        tr("No printer or paper profile is installed yet: load one, or get your paper's below."),
    ]
}

pub fn profile_note(chosen: &OutputProfile, installed: &[OutputProfile]) -> &'static str {
    if !chosen.is_usable() {
        troubles()[0]
    } else if installed.is_empty() {
        troubles()[1]
    } else {
        ""
    }
}

fn waiting(space: &Space) -> String {
    tr!("Until a profile is chosen, the picture stays in {space}.", space = space.title())
}

/// The one note under the space's tabs: a profile's trouble, the choice still to make, else what the
/// space is for.
pub fn space_note(space: &Space, pending: bool, installed: &[OutputProfile]) -> String {
    let trouble = match space {
        Space::Profile(chosen) => profile_note(chosen, installed),
        _ => "",
    };
    if !trouble.is_empty() {
        trouble.to_owned()
    } else if pending {
        waiting(space)
    } else {
        purpose(space).to_owned()
    }
}

/// Every note `space_note` may say, whatever the space, the profile or the choice still to make.
pub fn space_notes() -> Vec<String> {
    let waits = Space::BUILT_IN.iter().map(waiting);
    purposes().into_iter().chain(troubles()).map(str::to_owned).chain(waits).collect()
}

/// The space the photos were edited in, said whatever the space chosen so the note keeps its place,
/// and that the canvas and the previews, which share the export's space, show them in another.
pub fn edited_in_note(edited_in: Option<&Space>, space: &Space, batch: bool) -> String {
    let Some(edited_in) = edited_in else { return String::new() };
    let edited = edited_in.title();
    match (edited_in == space, batch) {
        (true, false) => tr!("This photo was edited in {edited}, the space the export writes.", edited = edited),
        (true, true) => tr!("These photos were edited in {edited}, the space the export writes.", edited = edited),
        (false, false) => tr!(
            "This photo was edited in {edited}. The canvas and this preview now show it in the space the export writes.",
            edited = edited
        ),
        (false, true) => tr!(
            "These photos were edited in {edited}. The canvas and these previews now show them in the space the export writes.",
            edited = edited
        ),
    }
}

/// Every note `edited_in_note` may say on this surface, in its space or another: of one photo, and of
/// a batch once `several` are exported, which Contact Sheet turns into one.
pub fn edited_in_notes(edited_in: Option<&Space>, several: bool) -> Vec<String> {
    let Some(edited_in) = edited_in else { return Vec::new() };
    let other = if *edited_in == Space::Srgb { Space::DisplayP3 } else { Space::Srgb };
    let batches: &[bool] = if several { &[false, true] } else { &[false] };
    [edited_in, &other].into_iter().flat_map(|space| batches.iter().map(move |batch| edited_in_note(Some(edited_in), space, *batch))).collect()
}

/// `texts` handed to the window only when they differ from `shown`, so a notch never lays them again.
pub(super) fn changed(shown: ModelRc<SharedString>, texts: Vec<String>) -> Option<ModelRc<SharedString>> {
    let same = shown.row_count() == texts.len() && texts.iter().enumerate().all(|(i, t)| shown.row_data(i).is_some_and(|s| s == t.as_str()));
    (!same).then(|| ModelRc::new(VecModel::from(texts.into_iter().map(SharedString::from).collect::<Vec<_>>())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_note_the_field_says_is_among_those_its_room_is_fitted_to() {
        let notes = space_notes();
        let gone = OutputProfile { path: "/nowhere/Gone.icc".into(), name: "Gone".into() };
        let states = Space::BUILT_IN.into_iter().flat_map(|s| [(s.clone(), false), (s, true)]).chain([(Space::Profile(gone.clone()), false)]);
        let said: Vec<String> = states.flat_map(|(space, pending)| [space_note(&space, pending, &[]), space_note(&space, pending, &[gone.clone()])]).collect();
        assert!(said.iter().all(|n| notes.contains(n)), "{said:?} against {notes:?}");
        assert!(said.contains(&purpose(&Space::ProPhoto).to_owned()) && said.contains(&troubles()[0].to_owned()));
        // Twin: a list fitted without the notes waiting for a profile misses one the field says.
        let without: Vec<&String> = notes.iter().filter(|n| !n.starts_with("Until")).collect();
        assert!(said.iter().any(|n| !without.contains(&n)));
    }

    #[test]
    fn the_edited_in_note_is_said_in_every_space_and_its_room_holds_each() {
        let p3 = Space::DisplayP3;
        assert_eq!(edited_in_note(Some(&p3), &p3, false), "This photo was edited in Display P3, the space the export writes.");
        assert_eq!(
            edited_in_note(Some(&Space::Srgb), &p3, true),
            "These photos were edited in sRGB. The canvas and these previews now show them in the space the export writes."
        );
        let notes = edited_in_notes(Some(&p3), true);
        for space in Space::BUILT_IN {
            for batch in [false, true] {
                assert!(notes.contains(&edited_in_note(Some(&p3), &space, batch)));
            }
        }
        // Twin: one photo never says a batch's words, so its room is not fitted to them.
        let alone = edited_in_notes(Some(&p3), false);
        assert!(alone.len() == 2 && !alone.contains(&edited_in_note(Some(&p3), &Space::Srgb, true)));
        // Before the surface opened nothing was edited, and nothing is said.
        assert_eq!(edited_in_note(None, &p3, false), "");
        assert!(edited_in_notes(None, true).is_empty());
    }
}
