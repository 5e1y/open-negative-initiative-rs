//! The phone remote on the store: each press from `oni-link`'s Host becomes a store command, and
//! the scanner's screen is published after every change, from whatever hand.

use crate::command::Command;
use crate::store::{EditorStore, Surface, channel, machine_for};
use oni_link::{Host, HostEvent, HostEvents, PublicKey, State};
use std::sync::Mutex;

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
}

/// Holds the Host and the last screen it published, so an unchanged screen is never resent.
pub struct Link {
    host: Host,
    pub frame: Option<(u16, u16)>,
    published: Option<State>,
}

impl Link {
    pub fn new(host: Host) -> Link {
        Link { host, frame: None, published: None }
    }

    /// Publishes the store's screen when it differs from the last one sent.
    pub fn refresh(&mut self, store: &EditorStore) {
        let state = screen(store, self.frame);
        if self.published.as_ref() != Some(&state) {
            self.host.publish(state.clone());
            self.published = Some(state);
        }
    }

    /// One event: a press lands on the store as one command, then the screen goes out.
    pub fn handle(&mut self, store: &mut EditorStore, event: HostEvent) -> Option<Outward> {
        let out = match event {
            HostEvent::Command { command, .. } => {
                store.apply(Command::from(command)).walk.map(Outward::Walk)
            }
            HostEvent::Paired { phone, name } => Some(Outward::Paired { phone, name }),
            HostEvent::Connected { phone, name } => Some(Outward::Connected { phone, name }),
            HostEvent::Disconnected { phone } => Some(Outward::Disconnected { phone }),
        };
        // Published after every press, changed or not, so the phone learns its press was taken.
        self.published = None;
        self.refresh(store);
        out
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
    while let Some(event) = events.next().await {
        let done = {
            let mut store = store.lock().expect("store lock");
            link.lock().expect("link lock").handle(&mut store, event)
        };
        if let Some(out) = done {
            outward(out);
        }
    }
}
