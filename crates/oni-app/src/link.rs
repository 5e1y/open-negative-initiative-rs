//! The phone remote on the store: each press from `oni-link`'s Host becomes a store command, the screen published after
//! every change; on the one face the wire's keypad, machine and face keys move nothing, its screen reading them at rest.

use oni_core::command::Command;
use oni_core::store::EditorStore;
use oni_link::{Channel, Face, Host, HostEvent, HostEvents, Machine, Panel, Param, PublicKey, Receipt, Row, State};
use oni_settings::{ConversionMode, PipelineSettings};
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

/// The phone's three channels: a digital photo's negative moves as a negative does.
fn channel(mode: ConversionMode) -> Channel {
    match mode {
        ConversionMode::Negative | ConversionMode::Digital => Channel::Negative,
        ConversionMode::Positive => Channel::Positive,
        ConversionMode::Monochrome => Channel::Monochrome,
    }
}

/// The screen as the store holds it, on Pro's face, the keypad at rest; `frame` is the open frame's place in its group.
pub fn screen(store: &EditorStore, frame: Option<(u16, u16)>) -> State {
    let s: &PipelineSettings = store.settings();
    let open = store.path().is_some();
    State {
        revision: store.revision(),
        applied: 0,
        frame: if open { frame } else { None },
        machine: Machine::Oni,
        channel: channel(s.mode),
        clicks: oni_link::Clicks::default(),
        contrast: s.print.contrast,
        vignette: s.vignette.stops,
        chrome: (!s.is_monochrome()).then_some(s.secret_sauce),
        remeasure: false,
        can_undo: store.can_undo(),
        can_redo: store.can_redo(),
        panel: Panel { face: Face::Pro, rows: Vec::new() },
    }
}

/// What the host app does with an event the store cannot: trust a phone, or open another frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Outward {
    Paired { phone: PublicKey, name: String },
    Connected { phone: PublicKey, name: String, connection: u64 },
    Disconnected { phone: PublicKey, connection: u64 },
    Walk(i8),
}

/// Holds the Host and the last screen it published, so an unchanged screen is never resent.
pub struct Link {
    host: Host,
    pub frame: Option<(u16, u16)>,
    pub sliders: Box<dyn Sliders>,
    /// The host's screen covering the editor: a press lands nowhere until it goes.
    pub covered: bool,
    published: Option<State>,
    /// The slider a phone's hand holds, and the connection it holds it on: the store records its
    /// moves as one step on release.
    held: Option<(Param, u64)>,
}

impl Link {
    pub fn new(host: Host) -> Link {
        Link { host, frame: None, sliders: Box::new(NoSliders), covered: false, published: None, held: None }
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
            HostEvent::Command { command, connection, .. } => self.command(store, command, connection),
            HostEvent::Paired { phone, name } => Some(Outward::Paired { phone, name }),
            HostEvent::Connected { phone, name, connection } => Some(Outward::Connected { phone, name, connection }),
            // A phone's old link ending after its reconnection leaves the new link's drag alone.
            HostEvent::Disconnected { phone, connection } => {
                if self.held.is_some_and(|(_, on)| on == connection) {
                    self.let_go(store);
                }
                Some(Outward::Disconnected { phone, connection })
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

    fn command(&mut self, store: &mut EditorStore, command: oni_link::Command, connection: u64) -> Option<Outward> {
        if self.covered {
            self.let_go(store);
            return None;
        }
        // `DSSlider`'s drag: the first place holds the store, every later one lands under that hold.
        if let oni_link::Command::SetParam { param, position } = command {
            let set = self.sliders.command(store, param, Move::To(position))?;
            if self.held != Some((param, connection)) {
                self.let_go(store);
                store.hold();
                self.held = Some((param, connection));
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
            key => command_of(key).ok().flatten(),
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

/// A phone's key as the store's command, `None` where the one face has no field for it; a panel slider's step stays
/// the wire's, for the host app that knows its sliders to resolve.
pub fn command_of(command: oni_link::Command) -> Result<Option<Command>, oni_link::Command> {
    use oni_link::Command as Link;
    Ok(match command {
        Link::Walk { by } => Some(Command::Walk { by }),
        Link::Undo => Some(Command::Undo),
        Link::Redo => Some(Command::Redo),
        Link::Step { .. } | Link::Reset { .. } | Link::StepEffect { .. } | Link::ResetEffect { .. } | Link::Machine(_) | Link::Face(_) => None,
        Link::StepParam { .. } | Link::ResetParam { .. } | Link::SetParam { .. } | Link::ReleaseParam { .. } => return Err(command),
    })
}

/// The wire's form of a command, where the phone has a key for it.
pub fn link_of(command: &Command) -> Option<oni_link::Command> {
    use oni_link::Command as Link;
    Some(match *command {
        Command::Walk { by } => Link::Walk { by },
        Command::Undo => Link::Undo,
        Command::Redo => Link::Redo,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_link::{Effect, Letter};

    #[test]
    fn the_walk_and_history_keys_cross_into_the_store_and_back_and_the_keypad_lands_nowhere() {
        for key in [oni_link::Command::Walk { by: 1 }, oni_link::Command::Undo, oni_link::Command::Redo] {
            assert_eq!(link_of(&command_of(key).unwrap().unwrap()), Some(key));
        }
        let keypad = [
            oni_link::Command::Step { letter: Letter::M, by: -2 },
            oni_link::Command::Reset { letter: Letter::W },
            oni_link::Command::StepEffect { effect: Effect::Chrome, by: 1 },
            oni_link::Command::ResetEffect { effect: Effect::Vignette },
            oni_link::Command::Machine(Machine::Frontier),
            oni_link::Command::Face(Face::Scanner),
        ];
        assert!(keypad.into_iter().all(|k| command_of(k) == Ok(None)), "no field of the one face answers the keypad");
        // Twin: a desktop-only command has no key on the phone, and a slider's step no store command.
        assert_eq!(link_of(&Command::Reset), None);
        let step = oni_link::Command::StepParam { param: oni_link::Param::Exposure, by: 1 };
        assert_eq!(command_of(step), Err(step));
    }
}
