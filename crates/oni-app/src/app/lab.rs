//! The Scanner face in the window (`EditorScene+Lab`): the face switch, the keypad's presses sent as
//! the phone's own commands, the Lab screen, and the keys the face answers.

use super::Ctl;
use crate::{Bar, Lab, LabCell, MainWindow, PanelState};
use oni_core::Command;
use oni_link::Command as Link;
use oni_link::scanner::{Effect, Letter, Machine};
use oni_settings::blocks::Vignette;
use oni_settings::{ConversionMode, Face, PipelineSettings, ScannerMachine};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// `@AppStorage("lastFaceScanner")`: the face the editor opens on, kept across launches.
const REMEMBERED: &str = "lastFaceScanner";
const ESCAPE: &str = "\u{1b}";
const SPACE: &str = " ";
const UP: &str = "\u{F700}";
const DOWN: &str = "\u{F701}";
const LEFT: &str = "\u{F702}";
const RIGHT: &str = "\u{F703}";

/// The finishing keys' order on the panel, `ColorPane`'s.
pub const EFFECTS: [Effect; 3] = [Effect::Contrast, Effect::Vignette, Effect::Chrome];
pub const MACHINES: [Machine; 2] = [Machine::Oni, Machine::Frontier];

fn remembered_at(support: &Path) -> PathBuf {
    support.join(REMEMBERED)
}

/// The face last left, read from the support folder; Pro when it was never written.
pub fn remembered(support: &Path) -> Face {
    match std::fs::read_to_string(remembered_at(support)) {
        Ok(text) if text.trim() == "1" => Face::Scanner,
        _ => Face::Pro,
    }
}

fn remember(support: &Path, face: Face) {
    let _ = std::fs::create_dir_all(support);
    let _ = std::fs::write(remembered_at(support), if face == Face::Scanner { "1" } else { "0" });
}

/// `LabKeyboard.legend`: the outer keys their count, the inner ones their sign.
pub fn legend(step: i32) -> String {
    let sign = if step < 0 { "−" } else { "+" };
    if step.abs() == 1 { sign.to_string() } else { format!("{sign}{}", step.abs()) }
}

/// `LabScreen.spelled`: a letter's count with its sign, zero bare.
pub fn spelled(steps: i64) -> String {
    match steps {
        0 => "0".to_string(),
        s if s < 0 => format!("−{}", s.abs()),
        s => format!("+{s}"),
    }
}

/// `LabStepKeys.percent`: whole percent, signed, zero bare.
pub fn percent(value: f32) -> String {
    let shown = (value * 100.0).round() as i64;
    match shown {
        0 => "0 %".to_string(),
        s if s < 0 => format!("−{} %", s.abs()),
        s => format!("+{s} %"),
    }
}

/// `LabStepKeys.stops`: tenths of a stop, signed, under half a hundredth bare.
pub fn stops(value: f32) -> String {
    if value.abs() < 0.005 {
        return "0 EV".to_string();
    }
    format!("{}{:.1} EV", if value < 0.0 { "−" } else { "+" }, value.abs())
}

/// What the scanner's screen shows, every line as `LabScreen` writes it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Screen {
    pub frame: String,
    pub machine: String,
    pub remeasure: bool,
    pub cells: Vec<(String, String)>,
    pub finish: Vec<String>,
}

pub fn screen(s: &PipelineSettings, place: Option<(u16, u16)>) -> Screen {
    let channel = match s.mode {
        ConversionMode::Negative => "NEGA",
        ConversionMode::Positive => "POSI",
        ConversionMode::Monochrome => "B&W",
    };
    let clicks = &s.scanner;
    let counts = [clicks.d, clicks.c, clicks.m, clicks.y, clicks.w];
    let mut finish = vec![format!("CT {}", percent(s.scanner_finish.contrast)), format!("VIG {}", stops(s.scanner_finish.vignette.stops))];
    if !s.mode.is_monochrome() {
        finish.push(format!("CHR {}", percent(s.scanner_finish.secret_sauce)));
    }
    Screen {
        frame: place.map_or("FRAME --/--".to_string(), |(at, count)| format!("FRAME {at:02}/{count:02}")),
        machine: format!("{} · {channel}", s.scanner_machine.title().to_uppercase()),
        remeasure: s.roll_reference.as_ref().is_some_and(|r| r.median.is_none()),
        cells: Letter::ALL.iter().zip(counts).map(|(l, n)| (l.title().to_string(), spelled(n))).collect(),
        finish,
    }
}

/// The keypad's blocks, each with its own reset: `LabKeyboard.Group`, then the face's Effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Density,
    Colour,
    Look,
    Effects,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Density, Group::Colour, Group::Look, Group::Effects];

    pub fn letters(self) -> &'static [Letter] {
        match self {
            Group::Density => &[Letter::D],
            Group::Colour => &[Letter::C, Letter::M, Letter::Y],
            Group::Look => &[Letter::W],
            Group::Effects => &[],
        }
    }

    /// Whether its reset key shows: a letter off zero, or the face's finishing off `Effects()`.
    pub fn modified(self, s: &PipelineSettings) -> bool {
        let f = &s.scanner_finish;
        match self {
            Group::Effects => f.contrast != 0.0 || f.vignette != Vignette::default() || f.secret_sauce != 0.0,
            _ => self.letters().iter().any(|l| count(s, *l) != 0),
        }
    }

    /// The block back to rest as one write: its letters alone, or the face's Effects through its view.
    pub fn reset(self, settings: &PipelineSettings, view: &PipelineSettings) -> Command {
        if self == Group::Effects {
            let mut next = view.clone();
            (next.print.contrast, next.vignette, next.secret_sauce) = (0.0, Vignette::default(), 0.0);
            return Command::Finish(Box::new(next));
        }
        let mut next = settings.clone();
        for letter in self.letters() {
            match letter {
                Letter::D => next.scanner.d = 0,
                Letter::C => next.scanner.c = 0,
                Letter::M => next.scanner.m = 0,
                Letter::Y => next.scanner.y = 0,
                Letter::W => next.scanner.w = 0,
            }
        }
        Command::Write(Box::new(next))
    }
}

fn count(s: &PipelineSettings, letter: Letter) -> i64 {
    match letter {
        Letter::D => s.scanner.d,
        Letter::C => s.scanner.c,
        Letter::M => s.scanner.m,
        Letter::Y => s.scanner.y,
        Letter::W => s.scanner.w,
    }
}

fn at<T: Copy>(all: &[T], i: i32) -> Option<T> {
    all.get(usize::try_from(i).ok()?).copied()
}

impl Ctl {
    /// The face last left, laid on the store before the first frame opens.
    pub(super) fn adopt_remembered_face(&self) {
        let support = self.state.borrow().support.clone();
        self.state.borrow_mut().store.apply(Command::Face(remembered(&support)));
    }

    /// `toggleLab` and `leaveLab`: the Scanner face needs a photograph, leaving needs a Lab.
    fn set_face(self: &Rc<Self>, face: Face) {
        let (open, current, support) = {
            let s = self.state.borrow();
            (s.open.is_some(), s.store.face(), s.support.clone())
        };
        if face == current || (face == Face::Scanner && !open) {
            return self.publish_bar();
        }
        remember(&support, face);
        self.write(Command::Face(face));
    }

    /// Whether the Scanner face is on screen: the face, a photograph worked, not the gallery.
    fn in_lab(&self) -> bool {
        let gallery = self.window().is_some_and(|w| w.global::<crate::App>().get_gallery());
        !gallery && self.state.borrow().store.face() == Face::Scanner
    }

    /// A key of the desktop keypad, as the phone's command of the same name.
    fn press(self: &Rc<Self>, link: Link) {
        match link {
            Link::Walk { by } => self.walk(by),
            _ => self.write(Command::from(link)),
        }
    }

    /// Bar keys and menu items the Scanner face answers; anything else is left to `act`.
    pub(super) fn lab_act(self: &Rc<Self>, action: &str) -> bool {
        match action {
            "face-scanner" => self.set_face(Face::Scanner),
            "face-pro" => self.set_face(Face::Pro),
            // `live(.emulator)`: a photograph worked, never from the gallery.
            "emulator" => {
                let gallery = self.window().is_some_and(|w| w.global::<crate::App>().get_gallery());
                if !gallery {
                    self.set_face(if self.in_lab() { Face::Pro } else { Face::Scanner });
                }
            }
            _ => return false,
        }
        true
    }

    /// `labEscape` and `walkKeys`: Esc leaves the Lab, a bare arrow walks the open frame's group,
    /// a bare Space goes to the gallery; a held Esc or Space acts once.
    pub(super) fn lab_key(self: &Rc<Self>, text: &str, bare: bool, repeat: bool) -> bool {
        let gallery = self.window().is_some_and(|w| w.global::<crate::App>().get_gallery());
        if gallery {
            return false;
        }
        let dedust = self.window().is_some_and(|w| w.global::<PanelState>().get_tab() == 4);
        match text {
            ESCAPE if self.in_lab() && !repeat => self.set_face(Face::Pro),
            UP | LEFT if bare => self.walk(-1),
            DOWN | RIGHT if bare => self.walk(1),
            // `walkKeys`: Space reaches the gallery from Pro's editor only, never from the Lab or De-dust.
            SPACE if bare && !repeat && !self.in_lab() && !dedust => self.act("toggle-mode"),
            _ => return false,
        }
        true
    }

    /// The Lab's screen, keypad state and face, read before any is set; a model is rebuilt only
    /// when its rows changed, so a notch elsewhere leaves the screen's repeaters alone.
    pub(super) fn publish_lab(&self) {
        let Some(w) = self.window() else { return };
        let gallery = w.global::<crate::App>().get_gallery();
        let (face, settings, view_place) = {
            let s = self.state.borrow();
            (s.store.face(), s.store.settings().clone(), s.place())
        };
        let shown = face == Face::Scanner && !gallery;
        let screen = screen(&settings, view_place);
        let modified: Vec<bool> = Group::ALL.iter().map(|g| g.modified(&settings)).collect();
        let lab = w.global::<Lab>();
        w.global::<Bar>().set_face(i32::from(face == Face::Scanner));
        lab.set_shown(shown);
        lab.set_machine(i32::from(settings.scanner_machine == ScannerMachine::Frontier));
        lab.set_monochrome(settings.mode.is_monochrome());
        lab.set_frame(screen.frame.into());
        lab.set_machine_line(screen.machine.into());
        lab.set_remeasure(screen.remeasure);
        let cells: Vec<LabCell> = screen.cells.into_iter().map(|(title, value)| LabCell { title: title.into(), value: value.into() }).collect();
        if !same(&lab.get_cells(), &cells) {
            lab.set_cells(ModelRc::new(VecModel::from(cells)));
        }
        let finish: Vec<SharedString> = screen.finish.into_iter().map(SharedString::from).collect();
        if !same(&lab.get_finish(), &finish) {
            lab.set_finish(ModelRc::new(VecModel::from(finish)));
        }
        if !same(&lab.get_modified(), &modified) {
            lab.set_modified(ModelRc::new(VecModel::from(modified)));
        }
        let separation = !settings.mode.is_monochrome() && settings.rgb_passes.is_none() && !shown;
        w.global::<PanelState>().set_separation_shown(separation);
    }

    pub(super) fn wire_lab(me: &Rc<Ctl>, window: &MainWindow) {
        let lab = window.global::<Lab>();
        let legends: Vec<SharedString> = [-3, -1, 1, 3].map(|s| legend(s).into()).to_vec();
        lab.set_legends(ModelRc::new(VecModel::from(legends)));
        let p = me.clone();
        lab.on_press(move |letter, by| {
            if let (Some(letter), Ok(by)) = (at(&Letter::ALL, letter), i8::try_from(by)) {
                p.press(Link::Step { letter, by });
            }
        });
        let p = me.clone();
        lab.on_clear(move |letter| {
            if let Some(letter) = at(&Letter::ALL, letter) {
                p.press(Link::Reset { letter });
            }
        });
        let p = me.clone();
        lab.on_step_effect(move |effect, by| {
            if let (Some(effect), Ok(by)) = (at(&EFFECTS, effect), i8::try_from(by)) {
                p.press(Link::StepEffect { effect, by });
            }
        });
        let p = me.clone();
        lab.on_clear_effect(move |effect| {
            if let Some(effect) = at(&EFFECTS, effect) {
                p.press(Link::ResetEffect { effect });
            }
        });
        let p = me.clone();
        lab.on_machine_picked(move |machine| {
            if let Some(machine) = at(&MACHINES, machine) {
                p.press(Link::Machine(machine));
            }
        });
        let p = me.clone();
        lab.on_reset_group(move |group| {
            let Some(group) = at(&Group::ALL, group) else { return };
            let command = {
                let s = p.state.borrow();
                group.reset(s.store.settings(), &s.store.face_view())
            };
            p.write(command);
        });
    }
}

fn same<T: Clone + PartialEq + 'static>(model: &ModelRc<T>, rows: &[T]) -> bool {
    model.row_count() == rows.len() && rows.iter().enumerate().all(|(i, r)| model.row_data(i).as_ref() == Some(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::scanner::ScannerClicks;

    #[test]
    fn the_screen_spells_every_count_and_finish_as_the_lab_does() {
        assert_eq!([-3, -1, 1, 3].map(legend), ["−3", "−", "+", "+3"]);
        assert_eq!((spelled(0), spelled(5), spelled(-12)), ("0".into(), "+5".into(), "−12".into()));
        assert_eq!((percent(0.004), percent(0.15), percent(-0.05)), ("0 %".into(), "+15 %".into(), "−5 %".into()));
        assert_eq!((stops(0.004), stops(0.3), stops(-1.25)), ("0 EV".into(), "+0.3 EV".into(), "−1.2 EV".into()));
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.scanner = ScannerClicks::new(3, -2, 0, 24, -10);
        s.scanner_finish.contrast = 0.2;
        s.scanner_machine = ScannerMachine::Frontier;
        let shown = screen(&s, Some((9, 38)));
        assert_eq!(shown.frame, "FRAME 09/38");
        assert_eq!(shown.machine, "FRONTIER · NEGA");
        let values: Vec<&str> = shown.cells.iter().map(|c| c.1.as_str()).collect();
        assert_eq!(values, ["+3", "−2", "0", "+24", "−10"]);
        assert_eq!(shown.cells[4].0, "TEMP");
        assert_eq!(shown.finish, ["CT +20 %", "VIG 0 EV", "CHR 0 %"]);
        // Twin: in black and white Chrome leaves the screen, and with no frame open the place is blank.
        s.mode = ConversionMode::Monochrome;
        let mono = screen(&s, None);
        assert_eq!((mono.frame.as_str(), mono.machine.as_str(), mono.finish.len()), ("FRAME --/--", "FRONTIER · B&W", 2));
    }

    #[test]
    fn a_group_s_reset_clears_its_own_letters_alone() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.scanner = ScannerClicks::new(3, -2, 1, 4, 5);
        let Command::Write(next) = Group::Colour.reset(&s, &s) else { panic!("a letter group writes the clicks") };
        assert_eq!(next.scanner, ScannerClicks::new(3, 0, 0, 0, 5));
        assert!(Group::Colour.modified(&s) && !Group::Colour.modified(&next));
        // Twin: Density's reset leaves the colour letters where they are.
        let Command::Write(next) = Group::Density.reset(&s, &s) else { panic!() };
        assert_eq!(next.scanner, ScannerClicks::new(0, -2, 1, 4, 5));
        assert!(!Group::Effects.modified(&s));
        s.scanner_finish.vignette.stops = 0.3;
        assert!(Group::Effects.modified(&s));
    }

    #[test]
    fn the_face_is_remembered_across_launches() {
        let dir = std::env::temp_dir().join(format!("oni-app-lab-face-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(remembered(&dir), Face::Pro, "never written, the editor opens on Pro");
        remember(&dir, Face::Scanner);
        assert_eq!(remembered(&dir), Face::Scanner);
        remember(&dir, Face::Pro);
        assert_eq!(remembered(&dir), Face::Pro);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
