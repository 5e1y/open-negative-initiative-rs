//! A photo the canvas cannot show, told before any decode in the hand's words, and the edits a
//! refused write kept as the hand left their photo, written again or let go at its word.

use super::*;
use oni_library::absence::{self, Absence};
use oni_library::navigator::Computer;
use oni_library::paths;

/// Why the open photo is not drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cause {
    Absent(Absence),
    /// One of a lead's passes, by its channel (0 red, 1 green, 2 blue) and its name.
    Pass { channel: usize, name: String, absence: Absence },
    Unsupported(String),
}

/// The cause that keeps `path` off the canvas, `None` for a file left to its decode. Reads the disk.
pub(crate) fn cause(computer: &Computer, path: &str, passes: Option<&oni_settings::frame::RgbPasses>) -> Option<Cause> {
    let lead = Path::new(path);
    if let Some(a) = absence::absence(computer, lead) {
        return Some(Cause::Absent(a));
    }
    if let Some(p) = passes {
        let folder = lead.parent().unwrap_or(Path::new(""));
        for (channel, name) in [&p.red, &p.green, &p.blue].into_iter().enumerate() {
            if let Some(a) = absence::absence(computer, &folder.join(name)) {
                return Some(Cause::Pass { channel, name: name.clone(), absence: a });
            }
        }
        return None;
    }
    oni_decode::unsupported_format(lead).map(Cause::Unsupported)
}

/// The canvas's title for `cause`, then what to do about it.
pub(crate) fn said(cause: &Cause, name: &str) -> (String, String) {
    match cause {
        Cause::Absent(Absence::Unplugged { volume }) => (
            tr!("“{name}” cannot be found", name = name),
            tr!("Its disk “{volume}” is not connected. Connect it to edit this photo; nothing was changed.", volume = volume),
        ),
        Cause::Absent(Absence::Gone) => {
            (tr!("“{name}” cannot be found", name = name), tr("It was moved or deleted: its disk is here, the file is not.").to_string())
        }
        Cause::Absent(Absence::Denied) => (
            tr!("ONI has no access to “{name}”", name = name),
            tr("Allow ONI to read its folder in the system's privacy settings, then open it again.").to_string(),
        ),
        Cause::Absent(Absence::Empty) => {
            (tr!("“{name}” is empty (0 bytes)", name = name), tr("An interrupted copy? Copy it again from its source.").to_string())
        }
        Cause::Absent(Absence::Unreadable(e)) => (tr!("“{name}” cannot be read", name = name), e.clone()),
        Cause::Unsupported(format) => (
            tr!("Format not supported ({format})", format = format),
            tr!("ONI cannot read “{name}”. Convert it to TIFF or JPEG to edit it.", name = name),
        ),
        Cause::Pass { channel, name: pass, absence } => {
            let empty = *absence == Absence::Empty;
            let detail = match (channel, empty) {
                (0, false) => tr!("Its red pass “{pass}” is missing beside it.", pass = pass),
                (1, false) => tr!("Its green pass “{pass}” is missing beside it.", pass = pass),
                (_, false) => tr!("Its blue pass “{pass}” is missing beside it.", pass = pass),
                (0, true) => tr!("Its red pass “{pass}” is empty (0 bytes).", pass = pass),
                (1, true) => tr!("Its green pass “{pass}” is empty (0 bytes).", pass = pass),
                (_, true) => tr!("Its blue pass “{pass}” is empty (0 bytes).", pass = pass),
            };
            (tr!("“{name}” cannot be shown", name = name), detail)
        }
    }
}

/// Why a sidecar refused the edits of `path`: the disk's reason in the hand's words when it is
/// one of the absences, else the disk's own message.
fn refusal(computer: &Computer, path: &str, message: &str) -> String {
    match absence::absence(computer, Path::new(path)) {
        Some(Absence::Unplugged { volume }) => tr!("Its disk “{volume}” is not connected.", volume = volume),
        Some(Absence::Gone) => tr("Its file was moved or deleted.").to_string(),
        _ => message.to_string(),
    }
}

impl Ctl {
    pub(in crate::app) fn wire_missing(me: &Rc<Ctl>, window: &MainWindow) {
        let a = window.global::<crate::Absent>();
        let weak = window.as_weak();
        a.on_show_folder(move || {
            if let Some(w) = weak.upgrade() {
                system::show_folder(Path::new(w.global::<crate::Absent>().get_folder().as_str()));
            }
        });
        let p = me.clone();
        a.on_back(move || p.act(Act::ToggleMode));
        let p = me.clone();
        window.global::<crate::Unsaved>().on_answered(move |i| p.unsaved_answered(i));
    }

    /// The open frame's original cannot be seen, so its sliders take no hand.
    pub(in crate::app) fn inert(&self) -> bool {
        self.state.borrow().store.editor_ref().is_some_and(|e| e.inert())
    }

    /// Lays on the canvas why `path` cannot be shown, else clears the note; true when it says so,
    /// and the decode is then never asked.
    pub(in crate::app) fn say_absent(&self, path: &str) -> bool {
        let found = {
            let s = self.state.borrow();
            cause(&s.nav.computer, path, s.store.settings().rgb_passes.as_ref())
        };
        let Some(w) = self.window() else { return found.is_some() };
        let a = w.global::<crate::Absent>();
        let Some(found) = found else {
            a.set_title("".into());
            return false;
        };
        let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (title, detail) = said(&found, &name);
        a.set_title(title.as_str().into());
        a.set_detail(detail.into());
        a.set_folder(absence::nearest_folder(path).into());
        self.clear_output();
        self.say(&title);
        true
    }

    /// The sheet over the window while edits a refused write kept wait in memory.
    pub(in crate::app) fn ask_unsaved(&self) {
        let Some(w) = self.window() else { return };
        let u = w.global::<crate::Unsaved>();
        let (count, first) = {
            let s = self.state.borrow();
            let kept = s.store.editor_ref().map_or(&[][..], |e| e.unsaved());
            let first = kept.first().map(|k| (k.path.clone(), refusal(&s.nav.computer, &k.path, &k.message)));
            (kept.len(), first)
        };
        let Some((path, reason)) = first else {
            return u.set_shown(false);
        };
        let title = match count {
            1 => tr!("The edits of “{name}” could not be saved", name = paths::name(&path)),
            n => trn("The edits of {n} photo could not be saved", "The edits of {n} photos could not be saved", n as u64),
        };
        u.set_title(title.into());
        u.set_message(tr!("{reason}\n\nONI keeps them while it runs and writes them as soon as the disk is back. Retry once the problem is fixed, or discard them.", reason = reason).into());
        if !u.get_shown() {
            u.set_shown(true);
            w.invoke_refocus();
        }
    }

    fn unsaved_answered(self: &Rc<Self>, i: i32) {
        match i {
            0 => self.state.borrow_mut().store.editor().into_iter().for_each(|e| e.discard_unsaved()),
            1 => return self.retry_unsaved(true),
            _ => {}
        }
        if let Some(w) = self.window() {
            w.global::<crate::Unsaved>().set_shown(false);
            w.invoke_refocus();
        }
    }

    /// Writes the kept edits again, at the hand's word or as a volume comes back; the sheet asks
    /// again while a write is refused, at once when the hand asked, and goes once all are written.
    pub(in crate::app) fn retry_unsaved(&self, asked: bool) {
        let Some(refused) = self.state.borrow_mut().store.editor().filter(|e| !e.unsaved().is_empty()).map(|e| e.retry_unsaved()) else { return };
        let shown = self.window().is_some_and(|w| w.global::<crate::Unsaved>().get_shown());
        match refused {
            Some(_) if asked || shown => self.ask_unsaved(),
            Some(_) => {}
            None => {
                if let Some(w) = self.window() {
                    w.global::<crate::Unsaved>().set_shown(false);
                }
                self.say(tr("The kept edits are saved."));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_cause_says_its_own_words_and_never_could_not_be_decoded() {
        let causes = [
            Cause::Absent(Absence::Unplugged { volume: "T7".into() }),
            Cause::Absent(Absence::Gone),
            Cause::Absent(Absence::Denied),
            Cause::Absent(Absence::Empty),
            Cause::Unsupported("HEIC".into()),
            Cause::Pass { channel: 0, name: "DSC_0102.NEF".into(), absence: Absence::Gone },
        ];
        let words: Vec<String> = causes.iter().map(|c| { let (t, d) = said(c, "DSC_0101.NEF"); format!("{t} {d}") }).collect();
        assert!(words[0].contains("“T7” is not connected"), "{}", words[0]);
        assert!(words[1].contains("moved or deleted"));
        assert!(words[2].contains("no access"));
        assert!(words[3].contains("0 bytes"));
        assert!(words[4].starts_with("Format not supported (HEIC)"));
        assert!(words[5].contains("Its red pass “DSC_0102.NEF” is missing beside it."));
        let mut distinct = words.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), words.len(), "one message per cause");
        // Twin: none of them is the decoder's word, kept for a real decode failure.
        assert!(words.iter().all(|w| !w.contains("could not be decoded")));
    }
}
