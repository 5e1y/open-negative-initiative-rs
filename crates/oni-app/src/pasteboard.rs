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

/// One board in memory shared by every holder of a clone, where the system has no typed board a
/// check may name: what one holder sets, another reads.
#[derive(Clone, Default)]
pub struct Shared(std::sync::Arc<std::sync::Mutex<Memory>>);

impl Board for Shared {
    fn change_count(&self) -> i64 {
        self.0.lock().map_or(0, |m| m.change_count())
    }

    fn data(&self, kind: &str) -> Option<Vec<u8>> {
        self.0.lock().ok()?.data(kind)
    }

    fn clear(&mut self) {
        if let Ok(mut m) = self.0.lock() {
            m.clear();
        }
    }

    fn set(&mut self, kind: &str, data: &[u8]) {
        if let Ok(mut m) = self.0.lock() {
            m.set(kind, data);
        }
    }
}

/// The system's general pasteboard; a process-wide one where the system has no typed board.
pub fn general() -> Box<dyn Board> {
    #[cfg(target_os = "macos")]
    return Box::new(mac::Pasteboard(None));
    #[cfg(target_os = "windows")]
    return Box::new(win::Clipboard);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Box::new(Memory::default())
}

/// The Windows clipboard under formats registered by the types' names, as the Mac's pasteboard
/// takes them, so a copy crosses windows and relaunches there too.
#[cfg(target_os = "windows")]
mod win {
    use clipboard_win::raw;

    pub struct Clipboard;

    fn format(kind: &str) -> Option<u32> {
        raw::register_format(kind).map(|f| f.get())
    }

    /// The clipboard opened for one operation, a few tries while another program holds it.
    fn opened() -> Option<clipboard_win::Clipboard> {
        clipboard_win::Clipboard::new_attempts(10).ok()
    }

    impl super::Board for Clipboard {
        fn change_count(&self) -> i64 {
            raw::seq_num().map_or(0, |n| i64::from(n.get()))
        }

        fn data(&self, kind: &str) -> Option<Vec<u8>> {
            let f = format(kind)?;
            let _open = opened()?;
            if !raw::is_format_avail(f) {
                return None;
            }
            let mut out = Vec::new();
            raw::get_vec(f, &mut out).ok()?;
            Some(out)
        }

        fn clear(&mut self) {
            if let Some(_open) = opened() {
                let _ = raw::empty();
            }
        }

        fn set(&mut self, kind: &str, data: &[u8]) {
            if let (Some(f), Some(_open)) = (format(kind), opened()) {
                let _ = raw::set_without_clear(f, data);
            }
        }
    }
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

    /// The Windows clipboard holds the app's own formats and never takes a text for settings; a
    /// runner whose clipboard will not open says so and checks nothing.
    #[cfg(target_os = "windows")]
    #[test]
    fn the_windows_clipboard_holds_the_apps_own_formats() {
        if clipboard_win::Clipboard::new_attempts(10).is_err() {
            return eprintln!("SKIPPED: the clipboard does not open here");
        }
        let mut board = general();
        write(&mut *board, SETTINGS, b"{\"version\":1}");
        assert_eq!(board.data(SETTINGS).as_deref(), Some(&b"{\"version\":1}"[..]));
        // Twin: a text put on the clipboard leaves no settings there.
        let _ = clipboard_win::set_clipboard_string("{\"version\":1}");
        assert_eq!(board.data(SETTINGS), None);
    }
}

