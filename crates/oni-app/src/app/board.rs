//! The store's copies on the system pasteboard: each copy written under its own type, the board
//! read back whenever its count has moved, so a copy made in another window or before a relaunch pastes.

use super::Ctl;
use crate::pasteboard::{self, FRAMING, MEASURE_WINDOW, SETTINGS};

impl Ctl {
    /// `Propagation.hasSettings`' cache on the change count: the board read only once it moved.
    pub(super) fn board_in(&self) {
        let mut s = self.state.borrow_mut();
        let s = &mut *s;
        let count = s.pasteboard.change_count();
        if s.board_seen == Some(count) {
            return;
        }
        s.board_seen = Some(count);
        let settings = s.pasteboard.data(SETTINGS).and_then(|d| String::from_utf8(d).ok());
        let framing = s.pasteboard.data(FRAMING).and_then(|d| serde_json::from_slice(&d).ok());
        s.window_board = s.pasteboard.data(MEASURE_WINDOW).and_then(|d| serde_json::from_slice(&d).ok());
        if let Some(e) = s.store.editor() {
            e.board.settings = settings;
            e.board.framing = framing;
        }
    }

    /// `Propagation.copy`, `copyFraming`, `copyMeasureWindow`: the store's copy of that kind laid on
    /// the board, as `SettingsCodec` and `JSONEncoder` write it.
    pub(super) fn board_out(&self, kind: &str) {
        let mut s = self.state.borrow_mut();
        let s = &mut *s;
        let data = match kind {
            SETTINGS => s.store.editor().and_then(|e| e.board.settings.clone()).map(String::into_bytes),
            FRAMING => s.store.editor().and_then(|e| e.board.framing).and_then(|g| serde_json::to_vec(&g).ok()),
            _ => s.window_board.and_then(|w| serde_json::to_vec(&w).ok()),
        };
        let Some(data) = data else { return };
        pasteboard::write(&mut *s.pasteboard, kind, &data);
        s.board_seen = Some(s.pasteboard.change_count());
    }
}
