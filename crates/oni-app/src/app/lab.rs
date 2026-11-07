//! The Scanner face in the window (`EditorScene+Lab`): the face switch, the keypad's presses sent as
//! the phone's own commands, the Lab screen, and the keys the face answers.

use super::Ctl;
use crate::fields::Field;
use crate::{Bar, Lab, LabCell, MainWindow, PanelState};
use oni_core::{Command, Each};
use oni_i18n::tr;
use oni_link::Command as Link;
use oni_link::scanner::{Effect, Letter, Machine};
use oni_pipeline::scanner::on_oni_machine;
use oni_settings::blocks::Vignette;
use oni_settings::{ConversionMode, Face, PipelineSettings, ScannerMachine};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::path::{Path, PathBuf};
use std::rc::Rc;

mod sliders;

/// `@AppStorage("lastFaceScanner")`: the face the editor opens on, kept across launches.
const REMEMBERED: &str = "lastFaceScanner";
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

pub(super) fn remember(support: &Path, face: Face) {
    let _ = std::fs::create_dir_all(support);
    let _ = std::fs::write(remembered_at(support), if face == Face::Scanner { "1" } else { "0" });
}

pub use oni_link::scanner::spelled;

/// The Pro slider moving the same quantity, whose readout an effect shares on both faces.
pub fn field(effect: Effect) -> Field {
    match effect {
        Effect::Contrast => Field::Contrast,
        Effect::Vignette => Field::Vignette,
        Effect::Chrome => Field::Chrome,
    }
}

/// What the scanner's screen shows, every line as `LabScreen` writes it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Screen {
    pub frame: String,
    pub machine: String,
    pub remeasure: bool,
    pub cells: Vec<(String, String)>,
    pub balance: String,
}

/// The balance the face prints with, read off its own render's filters: Auto White Balance's
/// correction at the roll's strength, the one Pro value its keys leave acting, in Pro's readouts.
pub fn balance(s: &PipelineSettings) -> String {
    let machine = on_oni_machine(&PipelineSettings { face: Face::Scanner, ..s.clone() }).unwrap_or_default();
    let read = |f: Field| f.readout(f.knob().get(&machine));
    format!("{} · {}", read(Field::Temperature), read(Field::Tint))
}

/// The screen's words are the panel's own, in the machine's capitals.
pub fn screen(s: &PipelineSettings, place: Option<(usize, usize)>) -> Screen {
    let channel = match s.mode {
        ConversionMode::Negative => tr("Negative"),
        ConversionMode::Positive => tr("Positive"),
        ConversionMode::Monochrome => tr("B&W"),
    };
    let clicks = &s.scanner;
    let counts = [clicks.d, clicks.c, clicks.m, clicks.y, clicks.w];
    let (at, count) = place.map_or(("--".to_string(), "--".to_string()), |(at, count)| (format!("{at:02}"), format!("{count:02}")));
    Screen {
        frame: tr!("FRAME {at}/{count}", at = at, count = count),
        machine: format!("{} · {}", s.scanner_machine.title().to_uppercase(), channel.to_uppercase()),
        remeasure: s.roll_reference.as_ref().is_some_and(|r| r.median.is_none()),
        cells: Letter::ALL.iter().zip(counts).map(|(l, n)| (l.title().to_string(), spelled(n))).collect(),
        balance: balance(s).to_uppercase(),
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

    /// The block back to rest on each frame it lands on: its letters alone, or the face's Effects
    /// through the view of the face edited, the Scanner's when `scanner`.
    pub fn rest(self, scanner: bool) -> Each {
        Each::new(move |s| {
            if self == Group::Effects {
                let mut view = if scanner { s.as_scanner() } else { s.clone() };
                (view.print.contrast, view.vignette, view.secret_sauce) = (0.0, Vignette::default(), 0.0);
                return if scanner { s.storing_scanner_edit(&view) } else { view };
            }
            let mut next = s.clone();
            for letter in self.letters() {
                match letter {
                    Letter::D => next.scanner.d = 0,
                    Letter::C => next.scanner.c = 0,
                    Letter::M => next.scanner.m = 0,
                    Letter::Y => next.scanner.y = 0,
                    Letter::W => next.scanner.w = 0,
                }
            }
            next
        })
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
        let (open, current) = {
            let s = self.state.borrow();
            (s.open.is_some(), s.store.face())
        };
        if face == current || (face == Face::Scanner && !open) {
            return self.publish_bar();
        }
        self.faced(face);
        self.write(Command::Face(face));
    }

    /// The face turned, by the bar, the phone or an undo, kept for the next launch; on the Scanner
    /// face the Lab's screen takes the deck, and the deck its preview.
    pub(super) fn faced(&self, face: Face) {
        remember(&self.state.borrow().support, face);
        if face == Face::Scanner {
            self.clear_hover();
        }
    }

    /// Whether the Scanner face is on screen: the face, a photograph worked, not the gallery.
    pub(super) fn in_lab(&self) -> bool {
        let gallery = self.window().is_some_and(|w| w.global::<crate::App>().get_gallery());
        !gallery && self.state.borrow().store.face() == Face::Scanner
    }

    /// `leaveLab()`: Pro's face, from the Lab alone.
    pub(super) fn leave_lab(self: &Rc<Self>) {
        if self.in_lab() {
            self.set_face(Face::Pro);
        }
    }

    /// A key of the desktop keypad, as the phone's command of the same name.
    fn press(self: &Rc<Self>, link: Link) {
        match link {
            Link::Walk { by } => self.walk(by),
            _ => crate::link::command_of(link).into_iter().for_each(|c| self.write(c)),
        }
    }

    /// Bar keys and menu items the Scanner face answers.
    pub(super) fn lab_act(self: &Rc<Self>, action: crate::Act) {
        match action {
            crate::Act::FaceScanner => self.set_face(Face::Scanner),
            crate::Act::FacePro => self.set_face(Face::Pro),
            // `live(.emulator)`: a photograph worked, never from the gallery.
            _ => {
                let gallery = self.window().is_some_and(|w| w.global::<crate::App>().get_gallery());
                if !gallery {
                    self.set_face(if self.in_lab() { Face::Pro } else { Face::Scanner });
                }
            }
        }
    }

    /// `walkKeys`: a bare arrow walks the open frame's group, a bare Space goes to the gallery, once
    /// while held; Escape leaves the Lab by the editor's one rule.
    pub(super) fn lab_key(self: &Rc<Self>, text: &str, bare: bool, repeat: bool) -> bool {
        let gallery = self.window().is_some_and(|w| w.global::<crate::App>().get_gallery());
        if gallery {
            return false;
        }
        let dedust = self.window().is_some_and(|w| w.global::<PanelState>().get_tab() == 4);
        match text {
            UP | LEFT if bare => self.walk(-1),
            DOWN | RIGHT if bare => self.walk(1),
            // `walkKeys` and `columnKeys`: Space reaches the gallery from the Lab, and from Pro outside De-dust.
            SPACE if bare && !repeat && (self.in_lab() || !dedust) => self.act(crate::Act::ToggleMode),
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
        // A group's reset key shows while a frame shown stands off the rest it would take, the open one or one beside it.
        let beside = self.beside().groups;
        let modified: Vec<bool> = Group::ALL.iter().zip(beside).map(|(g, beside)| beside || g.modified(&settings)).collect();
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
        lab.set_balance(screen.balance.into());
        sliders::publish(&lab, &settings);
        if !same(&lab.get_modified(), &modified) {
            lab.set_modified(ModelRc::new(VecModel::from(modified)));
        }
        let separation = !settings.mode.is_monochrome() && settings.rgb_passes.is_none() && !settings.inverted_source;
        let panel = w.global::<PanelState>();
        panel.set_separation_shown(separation);
        panel.set_slide(settings.mode == oni_settings::ConversionMode::Positive);
    }

    pub(super) fn wire_lab(me: &Rc<Ctl>, window: &MainWindow) {
        let lab = window.global::<Lab>();
        let p = me.clone();
        lab.on_press(move |letter, by| {
            if let (Some(letter), Ok(by)) = (at(&Letter::ALL, letter), i8::try_from(by)) {
                p.press(Link::Step { letter, by });
            }
        });
        let p = me.clone();
        lab.on_hold(move |on| {
            let mut s = p.state.borrow_mut();
            if on {
                s.store.hold();
            } else {
                s.store.release();
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
            let scanner = p.state.borrow().store.face() == Face::Scanner;
            p.write(Command::Each(group.rest(scanner)));
        });
    }
}

fn same<T: Clone + PartialEq + 'static>(model: &ModelRc<T>, rows: &[T]) -> bool {
    model.row_count() == rows.len() && rows.iter().enumerate().all(|(i, r)| model.row_data(i).as_ref() == Some(r))
}

#[cfg(test)]
mod tests;
