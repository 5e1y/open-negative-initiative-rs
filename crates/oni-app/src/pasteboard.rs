//! `Propagation`'s board: the settings, the framing and the measuring window under the app's own
//! pasteboard types, so a copy crosses apps and relaunches and a text paste is never taken for one.

use std::collections::HashMap;

/// `Propagation.pasteboardType`.
pub const SETTINGS: &str = "app.opennegative.settings";
/// `Propagation.framingType`.
pub const FRAMING: &str = "app.opennegative.framing";
/// `Propagation.measureWindowType`.
pub const MEASURE_WINDOW: &str = "app.opennegative.measure-window";
pub const KINDS: [&str; 3] = [SETTINGS, FRAMING, MEASURE_WINDOW];

/// A pasteboard's four primitives, as `NSPasteboard` offers them.
pub trait Board {
    /// Bumped by every clear, so a reader knows when to read again.
    fn change_count(&self) -> i64;
    fn data(&self, kind: &str) -> Option<Vec<u8>>;
    fn clear(&mut self);
    fn set(&mut self, kind: &str, data: &[u8]);
}

/// Clearing is the only way to write the board, so the other copies are laid back after it.
pub fn write(board: &mut dyn Board, kind: &str, data: &[u8]) {
    let kept: Vec<(&str, Vec<u8>)> = KINDS.iter().filter(|k| **k != kind).filter_map(|k| Some((*k, board.data(k)?))).collect();
    board.clear();
    board.set(kind, data);
    for (other, payload) in kept {
        board.set(other, &payload);
    }
}

/// A board of this process alone: the checks', and the fallback where the system has no typed one.
#[derive(Default)]
pub struct Memory {
    items: HashMap<String, Vec<u8>>,
    count: i64,
}

impl Board for Memory {
    fn change_count(&self) -> i64 {
        self.count
    }

    fn data(&self, kind: &str) -> Option<Vec<u8>> {
        self.items.get(kind).cloned()
    }

    fn clear(&mut self) {
        self.items.clear();
        self.count += 1;
    }

    fn set(&mut self, kind: &str, data: &[u8]) {
        self.items.insert(kind.to_string(), data.to_vec());
    }
}

/// The system's general pasteboard; a process-wide one where the system has no typed board.
pub fn general() -> Box<dyn Board> {
    #[cfg(target_os = "macos")]
    return Box::new(mac::Pasteboard(None));
    #[cfg(not(target_os = "macos"))]
    Box::new(Memory::default())
}

/// A named system pasteboard, shared by every process asking that name: a check's own, which
/// crosses a relaunch without touching the user's clipboard.
#[cfg(target_os = "macos")]
pub fn named(name: &str) -> Box<dyn Board> {
    Box::new(mac::Pasteboard(Some(name.to_string())))
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSData, NSString};
    use std::ops::Deref;

    /// The general board, or the one of that name.
    pub struct Pasteboard(pub Option<String>);

    impl Pasteboard {
        fn board(&self) -> impl Deref<Target = NSPasteboard> {
            match &self.0 {
                Some(name) => NSPasteboard::pasteboardWithName(&NSString::from_str(name)),
                None => NSPasteboard::generalPasteboard(),
            }
        }
    }

    impl super::Board for Pasteboard {
        fn change_count(&self) -> i64 {
            self.board().changeCount() as i64
        }

        fn data(&self, kind: &str) -> Option<Vec<u8>> {
            self.board().dataForType(&NSString::from_str(kind)).map(|d| d.to_vec())
        }

        fn clear(&mut self) {
            self.board().clearContents();
        }

        fn set(&mut self, kind: &str, data: &[u8]) {
            self.board().setData_forType(Some(&NSData::with_bytes(data)), &NSString::from_str(kind));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_keeps_the_other_two_copies_and_bumps_the_count() {
        let mut board = Memory::default();
        write(&mut board, FRAMING, b"framing");
        write(&mut board, MEASURE_WINDOW, b"window");
        let before = board.change_count();
        write(&mut board, SETTINGS, b"settings");
        assert!(board.change_count() > before, "a reader sees the write");
        assert_eq!(board.data(SETTINGS).as_deref(), Some(&b"settings"[..]));
        assert_eq!(board.data(FRAMING).as_deref(), Some(&b"framing"[..]), "the framing's copy survives");
        assert_eq!(board.data(MEASURE_WINDOW).as_deref(), Some(&b"window"[..]));
        // Twin: a bare clear and set, as a text copy does, leaves the settings alone on the board.
        board.clear();
        board.set(SETTINGS, b"settings");
        assert_eq!(board.data(FRAMING), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_system_board_holds_the_apps_own_types_and_never_a_text() {
        let name = format!("oni-pasteboard-check-{}", std::process::id());
        let mut board = named(&name);
        write(&mut *board, SETTINGS, b"{\"version\":1}");
        let again = named(&name);
        assert_eq!(again.data(SETTINGS).as_deref(), Some(&b"{\"version\":1}"[..]), "another handle reads the same board");
        // Twin: a text on the board is not settings.
        board.clear();
        board.set("public.utf8-plain-text", b"{\"version\":1}");
        assert_eq!(again.data(SETTINGS), None);
    }
}
