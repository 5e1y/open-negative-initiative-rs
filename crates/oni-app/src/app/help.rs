//! The Help menu in the window: What's New's notes on the headings they changed, the welcome sheet
//! shown once per edition, Report a Bug sending its sealed log off this thread, and the console.

use super::{Ctl, with_shell};
use crate::{ConsoleWindow, Help, News, report, system};
use slint::{ComponentHandle, Timer, TimerMode};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// Sends one report: the message and, when ticked, the sealed log; a check hands its own.
pub type Reporter = Arc<dyn Fn(&str, Option<String>) -> Result<(), String> + Send + Sync>;

/// `WhatsNew.preferenceKey`: the notes a hover has read.
pub const SEEN_KEY: &str = "whatsNewSeen";
/// `WelcomeSheet.preferenceKey` and `edition`: a new edition shows the sheet to everyone again.
pub const WELCOME_KEY: &str = "welcomeSeen";
pub const EDITION: &str = "1.2.1";
const SITE: &str = "https://open-negative-initiative.com";
/// `DS.Motion.consoleRefresh`.
const CONSOLE_REFRESH: Duration = Duration::from_millis(500);

/// `WhatsNew.notes`: (id, word, note), in Swift's order.
pub const NOTES: [(&str, &str, &str); 6] = [
    ("machine.1.3.1", "New", "Two machines: the Frontier as measured, and ONI's own, Pro's conversion with ONI's look. A photo converted before keeps the Frontier."),
    ("keypad.1.3.1", "Updated", "A simpler Scanner, after your feedback: three blocks that fold, D, C, M and Y on half steps, W now TEMP. The S look and CMY CANCEL are gone."),
    ("effects.scanner.1.3.1", "Updated", "Contrast, Vignette and Chrome are five keys each, read on the machine's screen."),
    ("curves.scanner.1.3.1", "Updated", "Curves, Spectrogram and the saturation curve now act on the Scanner face too, after the machine's render."),
    ("lighting.1.3.1", "Updated", "Shadows and Highlights reach ±0.50, the curve still rising at the ends. Auto Exposure is gone; Auto White Balance stays."),
    ("space.1.3.1", "New", "ProPhoto RGB, the widest space, in 16-bit TIFF: it keeps colours Adobe RGB clips."),
];

/// `1.2` of `1.2.1`: the release an edition belongs to, as the menu and the sheet's heading name it.
pub fn release(edition: &str) -> &str {
    edition.match_indices('.').nth(1).map_or(edition, |(at, _)| &edition[..at])
}

/// The default reporter: a JSON post to the site's `/report`, as `BugReport.send` makes it.
pub fn post(message: &str, log: Option<String>) -> Result<(), String> {
    let body = report::body(message, env!("CARGO_PKG_VERSION"), log.as_deref());
    let code = crate::web::post_json(report::ENDPOINT, body)?;
    if (200..300).contains(&code) { Ok(()) } else { Err(report::failure(code)) }
}

impl Ctl {
    pub(super) fn wire_help(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.publish_news();
        window.global::<Help>().set_release(release(EDITION).into());
        let news = window.global::<News>();
        let p = me.clone();
        news.on_read(move |id| p.news_read(&id));
        let help = window.global::<Help>();
        let p = me.clone();
        help.on_welcome_closed(move || p.welcome_closed());
        help.on_website(|| system::open_url(SITE));
        let p = me.clone();
        help.on_feedback(move || {
            p.welcome_closed();
            p.open_report();
        });
        let p = me.clone();
        help.on_report_edited(move |text| {
            // `BugReport.messageLimit`: typing stops at the site's bound.
            if text.chars().count() > report::MESSAGE_LIMIT
                && let Some(w) = p.window()
            {
                w.global::<Help>().set_report_message(text.chars().take(report::MESSAGE_LIMIT).collect::<String>().into());
            }
        });
        let p = me.clone();
        help.on_report_send(move || p.send_report());
        let p = me.clone();
        help.on_report_closed(move || p.close_report());
        help.on_community(|| system::open_url(report::ISSUES));
    }

    /// Each heading's word while its note is news, read from what earlier sessions saw.
    fn publish_news(&self) {
        let Some(w) = self.window() else { return };
        let seen: Vec<String> = self.state.borrow().preferences.value(SEEN_KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        let shown = |id: &str| -> (slint::SharedString, slint::SharedString) {
            let note = NOTES.iter().find(|(i, _, _)| *i == id).filter(|(i, _, _)| !seen.iter().any(|s| s == i) || self.state.borrow().news_kept.contains(*i));
            note.map_or_else(Default::default, |(_, word, text)| (word.to_uppercase().into(), (*text).into()))
        };
        let n = w.global::<News>();
        let (word, note) = shown("machine.1.3.1");
        n.set_machine(word);
        n.set_machine_note(note);
        let (word, note) = shown("keypad.1.3.1");
        n.set_keypad(word);
        n.set_keypad_note(note);
        let (word, note) = shown("effects.scanner.1.3.1");
        n.set_effects_scanner(word);
        n.set_effects_scanner_note(note);
        let (word, note) = shown("curves.scanner.1.3.1");
        n.set_curves_scanner(word);
        n.set_curves_scanner_note(note);
        let (word, note) = shown("lighting.1.3.1");
        n.set_lighting(word);
        n.set_lighting_note(note);
        let (word, note) = shown("space.1.3.1");
        n.set_space(word);
        n.set_space_note(note);
    }

    /// `DSNewsBoard.read`: seen for the next launch, its pill kept until this one quits.
    fn news_read(&self, id: &str) {
        let written = {
            let mut s = self.state.borrow_mut();
            s.news_kept.insert(id.to_string());
            let mut seen: Vec<String> = s.preferences.value(SEEN_KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            if seen.iter().any(|x| x == id) {
                return;
            }
            seen.push(id.to_string());
            s.preferences.set(SEEN_KEY, seen.into())
        };
        if let Err(e) = written {
            self.say(&format!("The note cannot be marked read: {e}"));
        }
    }

    /// `showAgain`: every note news again, now and on the next launch.
    pub(super) fn show_whats_new_again(&self) {
        let removed = {
            let mut s = self.state.borrow_mut();
            s.news_kept.clear();
            s.preferences.set(SEEN_KEY, serde_json::Value::Array(Vec::new()))
        };
        if let Err(e) = removed {
            self.say(&format!("The notes cannot be shown again: {e}"));
        }
        self.publish_news();
    }

    /// `WelcomeSheet.isDue`: this edition not yet shown on this Mac.
    pub fn welcome_due(&self) -> bool {
        self.state.borrow().preferences.string(WELCOME_KEY) != Some(EDITION)
    }

    pub(super) fn open_welcome(&self) {
        let Some(w) = self.window() else { return };
        let version = env!("CARGO_PKG_VERSION");
        let h = w.global::<Help>();
        h.set_welcome_picture(crate::splash::picture(version) as i32);
        h.set_welcome_line(format!("V{version}").into());
        h.set_welcome_credit(crate::splash::credit(version).into());
        h.set_welcome_shown(true);
    }

    /// However it closes, it is not shown again on the next launch.
    fn welcome_closed(&self) {
        let Some(w) = self.window() else { return };
        if !w.global::<Help>().get_welcome_shown() {
            return;
        }
        w.global::<Help>().set_welcome_shown(false);
        let written = self.state.borrow_mut().preferences.set(WELCOME_KEY, EDITION.into());
        if let Err(e) = written {
            self.say(&format!("The welcome cannot be marked seen: {e}"));
        }
        w.invoke_refocus();
    }

    pub(super) fn open_report(&self) {
        let Some(w) = self.window() else { return };
        self.state.borrow_mut().report_round += 1;
        let h = w.global::<Help>();
        h.set_report_message(Default::default());
        h.set_report_log(true);
        h.set_report_sending(false);
        h.set_report_sent(false);
        h.set_report_failure(Default::default());
        h.set_report_shown(true);
    }

    fn close_report(&self) {
        if let Some(w) = self.window() {
            w.global::<Help>().set_report_shown(false);
            w.invoke_refocus();
        }
    }

    /// `ReportSheet.send`: sealed and posted off this thread; a failure stays on the sheet.
    fn send_report(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let h = w.global::<Help>();
        let message = h.get_report_message().to_string();
        if message.trim().is_empty() || h.get_report_sending() {
            return;
        }
        let with_log = h.get_report_log();
        h.set_report_sending(true);
        h.set_report_failure(Default::default());
        let (reporter, log, round) = {
            let s = self.state.borrow();
            (s.reporter.clone(), crate::timing::log_path(&s.support), s.report_round)
        };
        std::thread::spawn(move || {
            let sealed = with_log.then(|| {
                crate::timing::flush();
                report::sealed_log(&log)
            }).flatten();
            let outcome = reporter(&message, sealed);
            with_shell(move |c| {
                let Some(w) = c.window() else { return };
                if c.state.borrow().report_round != round {
                    return;
                }
                let h = w.global::<Help>();
                h.set_report_sending(false);
                match outcome {
                    Ok(()) => h.set_report_sent(true),
                    Err(e) => h.set_report_failure(e.into()),
                }
            });
        });
    }

    /// What the console window shows, while it is open.
    pub(crate) fn console_log(&self) -> Option<String> {
        self.state.borrow().console.as_ref().filter(|(c, _)| c.window().is_visible()).map(|(c, _)| c.get_log().to_string())
    }

    /// `openWindow(id: "console")`: one window, polled while it shows.
    pub(super) fn open_console(self: &Rc<Self>) {
        let existing = self.state.borrow().console.as_ref().map(|c| c.0.clone_strong());
        let console = match existing {
            Some(c) => c,
            None => {
                let Ok(console) = ConsoleWindow::new() else { return self.say("The console cannot open.") };
                let log = crate::timing::log_path(&self.state.borrow().support);
                let copied = log.clone();
                console.on_copy(move || system::set_text(&std::fs::read_to_string(&copied).unwrap_or_default()));
                let revealed = log.clone();
                console.on_reveal(move || system::reveal(&revealed));
                let timer = Timer::default();
                let weak = console.as_weak();
                // Read only while the window is up: closed, it costs nothing.
                timer.start(TimerMode::Repeated, CONSOLE_REFRESH, move || {
                    if let Some(c) = weak.upgrade().filter(|c| c.window().is_visible()) {
                        crate::timing::flush();
                        c.set_log(std::fs::read_to_string(&log).unwrap_or_default().into());
                        c.set_blocks(crate::watchdog::blocks() as i32);
                    }
                });
                self.state.borrow_mut().console = Some((console.clone_strong(), timer));
                console
            }
        };
        let log = crate::timing::log_path(&self.state.borrow().support);
        crate::timing::flush();
        console.set_log(std::fs::read_to_string(log).unwrap_or_default().into());
        console.set_blocks(crate::watchdog::blocks() as i32);
        let _ = console.show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notes_are_swifts_and_each_names_its_heading_once() {
        let ids: Vec<&str> = NOTES.iter().map(|(i, _, _)| *i).collect();
        assert_eq!(ids, ["machine.1.3.1", "keypad.1.3.1", "effects.scanner.1.3.1", "curves.scanner.1.3.1", "lighting.1.3.1", "space.1.3.1"]);
        assert!(NOTES.iter().all(|(_, w, _)| *w == "New" || *w == "Updated"));
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
        assert_eq!((SEEN_KEY, WELCOME_KEY, EDITION), ("whatsNewSeen", "welcomeSeen", "1.2.1"));
    }

    #[test]
    fn the_menu_and_the_heading_name_the_editions_release() {
        assert_eq!(release(EDITION), "1.2");
        assert_eq!(release("1.3"), "1.3");
        assert_eq!(release("2.0.4.1"), "2.0");
        // Twin: the edition itself, or the menu's former 1.1, is not what both name.
        assert_ne!(release(EDITION), EDITION);
        assert_ne!(release(EDITION), "1.1");
    }

    /// `WhatsNew.selfCheck`: every note hangs on a heading the interface draws, and every heading's
    /// id has its note, or a note is never seen and a heading draws nothing.
    #[test]
    fn every_note_hangs_on_a_heading_and_every_heading_has_its_note() {
        fn walk(dir: &std::path::Path, hung: &mut std::collections::BTreeSet<String>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, hung);
                } else if path.extension().is_some_and(|e| e == "slint") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    hung.extend(text.split("News.read(\"").skip(1).filter_map(|rest| rest.split('"').next().map(String::from)));
                }
            }
        }
        let mut hung = std::collections::BTreeSet::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui"), &mut hung);
        let noted: std::collections::BTreeSet<String> = NOTES.iter().map(|(i, _, _)| (*i).to_string()).collect();
        assert!(!hung.is_empty());
        assert_eq!(hung, noted, "each note on a heading, each heading's id noted");
        // Twin: a note named nowhere is caught.
        let mut ghost = noted.clone();
        ghost.insert("ghost.1.3.1".into());
        assert_eq!(ghost.difference(&hung).collect::<Vec<_>>(), ["ghost.1.3.1"]);
    }
}
