//! The phone remote on the store: each press from `oni-link`'s Host becomes a store command, and
//! the screen, the scanner's and the panel's, is published after every change, from whatever hand.

use oni_core::command::{Command, Scan};
use oni_core::store::{EditorStore, Surface, channel, machine_for};
use oni_link::{Face, Host, HostEvent, HostEvents, Panel, Param, PublicKey, Receipt, Row, State};
use oni_settings::Face as SettingsFace;
use std::sync::Mutex;

/// How a phone moves a slider: by hundredths of its track, back to rest, or to a place on its track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Move {
    Step(i8),
    Reset,
    To(f32),
}

/// The panel's sliders, which the host app lays out and formats: the store knows values, not sliders.
pub trait Sliders: Send {
    /// The rows the panel shows now on the open face, in its order.
    fn rows(&self, store: &EditorStore) -> Vec<Row>;
    /// The command a slider's move lands as.
    fn command(&self, store: &EditorStore, param: Param, by: Move) -> Option<Command>;
}

/// No sliders: the phone's tabs stay empty.
pub struct NoSliders;

impl Sliders for NoSliders {
    fn rows(&self, _: &EditorStore) -> Vec<Row> {
        Vec::new()
    }

    fn command(&self, _: &EditorStore, _: Param, _: Move) -> Option<Command> {
        None
    }
}

/// The scanner's screen as the store holds it; `frame` is the open frame's place in its group.
pub fn screen(store: &EditorStore, frame: Option<(u16, u16)>) -> State {
    let s = store.settings();
    let open = store.path().is_some();
    State {
        revision: store.revision(),
        applied: 0,
        frame: if open { frame } else { None },
        machine: machine_for(s.scanner_machine),
        channel: channel(s.mode),
        clicks: oni_link::Clicks {
            d: clamp(s.scanner.d),
            c: clamp(s.scanner.c),
            m: clamp(s.scanner.m),
            y: clamp(s.scanner.y),
            w: clamp(s.scanner.w),
        },
        contrast: s.scanner_finish.contrast,
        vignette: s.scanner_finish.vignette.stops,
        chrome: (!s.mode.is_monochrome()).then_some(s.scanner_finish.secret_sauce),
        remeasure: s.roll_reference.as_ref().is_some_and(|r| r.median.is_none()),
        can_undo: store.can_undo(Surface::Editor),
        can_redo: store.can_redo(Surface::Editor),
        panel: Panel { face: face_for(store.face()), rows: Vec::new() },
    }
}

fn clamp(clicks: i64) -> i8 {
    clicks.clamp(i64::from(i8::MIN), i64::from(i8::MAX)) as i8
}

/// What the host app does with an event the store cannot: trust a phone, or open another frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Outward {
    Paired { phone: PublicKey, name: String },
    Connected { phone: PublicKey, name: String },
    Disconnected { phone: PublicKey },
    Walk(i8),
    /// The face switched on the store, for the host to remember as its own switch does.
    Face(Face),
}

/// Holds the Host and the last screen it published, so an unchanged screen is never resent.
pub struct Link {
    host: Host,
    pub frame: Option<(u16, u16)>,
    pub sliders: Box<dyn Sliders>,
    published: Option<State>,
    /// The slider a phone's hand holds: the store records its moves as one step on release.
    held: Option<Param>,
}

impl Link {
    pub fn new(host: Host) -> Link {
        Link { host, frame: None, sliders: Box::new(NoSliders), published: None, held: None }
    }

    /// Publishes the store's screen when it differs from the last one sent.
    pub fn refresh(&mut self, store: &EditorStore) {
        let mut state = screen(store, self.frame);
        state.panel.rows = self.sliders.rows(store);
        if self.published.as_ref() != Some(&state) {
            self.host.publish(state.clone());
            self.published = Some(state);
        }
    }

    /// One event: a press lands on the store as one command, is marked taken, then the screen goes out.
    pub fn handle(&mut self, store: &mut EditorStore, event: HostEvent, receipt: Receipt) -> Option<Outward> {
        let out = match event {
            HostEvent::Command { command, .. } => self.command(store, command),
            HostEvent::Paired { phone, name } => Some(Outward::Paired { phone, name }),
            HostEvent::Connected { phone, name } => Some(Outward::Connected { phone, name }),
            HostEvent::Disconnected { phone } => {
                self.let_go(store);
                Some(Outward::Disconnected { phone })
            }
        };
        receipt.applied();
        // Published after every press, changed or not, so the phone learns its press was taken.
        self.published = None;
        self.refresh(store);
        out
    }

    /// The phone's hand off its slider, which records the drag as one step.
    pub fn let_go(&mut self, store: &mut EditorStore) {
        if self.held.take().is_some() {
            store.release();
        }
    }

    fn command(&mut self, store: &mut EditorStore, command: oni_link::Command) -> Option<Outward> {
        // `DSSlider`'s drag: the first place holds the store, every later one lands under that hold.
        if let oni_link::Command::SetParam { param, position } = command {
            let set = self.sliders.command(store, param, Move::To(position))?;
            if self.held != Some(param) {
                self.let_go(store);
                store.hold();
                self.held = Some(param);
            }
            store.apply(set);
            return None;
        }
        // Any other key ends a drag a lost release left open, so it never joins the drag's step.
        self.let_go(store);
        let resolved = match command {
            oni_link::Command::StepParam { param, by } => self.sliders.command(store, param, Move::Step(by)),
            oni_link::Command::ResetParam { param } => self.sliders.command(store, param, Move::Reset),
            oni_link::Command::SetParam { .. } | oni_link::Command::ReleaseParam { .. } => return None,
            // `toggleLab`: the Scanner face needs a photograph; the face already on is left alone.
            oni_link::Command::Face(face) => {
                let wanted = face_of(face);
                if wanted == store.face() || (wanted == SettingsFace::Scanner && store.path().is_none()) {
                    return None;
                }
                store.apply(Command::Face(wanted));
                return Some(Outward::Face(face));
            }
            key => command_of(key).ok(),
        };
        store.apply(resolved?).walk.map(Outward::Walk)
    }
}

/// Reads the Host's events until it closes, each under the store's lock; `outward` gets what the
/// host app must do itself.
pub async fn drive(
    mut events: HostEvents,
    link: &Mutex<Link>,
    store: &Mutex<EditorStore>,
    mut outward: impl FnMut(Outward),
) {
    while let Some((event, receipt)) = events.receive().await {
        let done = {
            let mut store = store.lock().expect("store lock");
            link.lock().expect("link lock").handle(&mut store, event, receipt)
        };
        if let Some(out) = done {
            outward(out);
        }
    }
}

/// A phone's key as the store's command; a panel slider's step stays the wire's, for the host app
/// that knows its sliders to resolve.
pub fn command_of(command: oni_link::Command) -> Result<Command, oni_link::Command> {
    {
        use oni_link::Command as Link;
        Ok(match command {
            Link::Step { letter, by } => Command::Scan(Scan::Step { letter, by }),
            Link::Reset { letter } => Command::Scan(Scan::Reset { letter }),
            Link::StepEffect { effect, by } => Command::Scan(Scan::StepEffect { effect, by }),
            Link::ResetEffect { effect } => Command::Scan(Scan::ResetEffect { effect }),
            Link::Machine(machine) => Command::Scan(Scan::Machine(machine)),
            Link::Walk { by } => Command::Scan(Scan::Walk { by }),
            Link::Undo => Command::Undo,
            Link::Redo => Command::Redo,
            Link::Face(face) => Command::Face(face_of(face)),
            Link::StepParam { .. } | Link::ResetParam { .. } | Link::SetParam { .. } | Link::ReleaseParam { .. } => {
                return Err(command);
            }
        })
    }
}

pub fn face_of(face: oni_link::Face) -> SettingsFace {
    match face {
        oni_link::Face::Pro => SettingsFace::Pro,
        oni_link::Face::Scanner => SettingsFace::Scanner,
    }
}

pub fn face_for(face: SettingsFace) -> oni_link::Face {
    match face {
        SettingsFace::Pro => oni_link::Face::Pro,
        SettingsFace::Scanner => oni_link::Face::Scanner,
    }
}

/// The wire's form of a command, where the phone has a key for it.
pub fn link_of(command: &Command) -> Option<oni_link::Command> {
    {
        use oni_link::Command as Link;
        Some(match *command {
            Command::Scan(Scan::Step { letter, by }) => Link::Step { letter, by },
            Command::Scan(Scan::Reset { letter }) => Link::Reset { letter },
            Command::Scan(Scan::StepEffect { effect, by }) => Link::StepEffect { effect, by },
            Command::Scan(Scan::ResetEffect { effect }) => Link::ResetEffect { effect },
            Command::Scan(Scan::Machine(machine)) => Link::Machine(machine),
            Command::Scan(Scan::Walk { by }) => Link::Walk { by },
            Command::Undo => Link::Undo,
            Command::Redo => Link::Redo,
            Command::Face(face) => Link::Face(face_for(face)),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_link::{Effect, Letter, Machine};

    #[test]
    fn every_phone_key_crosses_into_the_store_and_back_unchanged() {
        let keys = [
            oni_link::Command::Step { letter: Letter::M, by: -2 },
            oni_link::Command::Reset { letter: Letter::W },
            oni_link::Command::StepEffect { effect: Effect::Chrome, by: 1 },
            oni_link::Command::ResetEffect { effect: Effect::Vignette },
            oni_link::Command::Machine(Machine::Frontier),
            oni_link::Command::Walk { by: 1 },
            oni_link::Command::Undo,
            oni_link::Command::Redo,
            oni_link::Command::Face(oni_link::Face::Scanner),
        ];
        for key in keys {
            assert_eq!(link_of(&command_of(key).unwrap()), Some(key));
        }
        // Twin: a desktop-only command has no key on the phone, and a slider's step no store command.
        assert_eq!(link_of(&Command::Reset), None);
        let step = oni_link::Command::StepParam { param: oni_link::Param::Exposure, by: 1 };
        assert_eq!(command_of(step), Err(step));
    }
}
