//! The hand that drives the gallery's check: real pointer and key events, the menus' lines read back.

use oni_app::app;
use oni_app::Library;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::time::{Duration, Instant};

use crate::common;

pub(super) struct Hand<'a> {
    pub(super) rig: &'a common::Rig,
    pub(super) w: &'a oni_app::MainWindow,
    pub(super) popup: std::cell::Cell<bool>,
}

impl Hand<'_> {
    pub(super) fn centre(&self, caption: &str) -> (f32, f32) {
        let lib = self.w.global::<Library>();
        let t = app::tiles(self.w).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"));
        (lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0)
    }

    pub(super) fn event(&self, e: WindowEvent) {
        self.rig.window.dispatch_event(e);
        self.rig.pump();
    }

    pub(super) fn press_at(&self, (x, y): (f32, f32), button: PointerEventButton) {
        let position = LogicalPosition::new(x, y);
        self.event(WindowEvent::PointerMoved { position });
        self.event(WindowEvent::PointerPressed { position, button });
    }

    pub(super) fn release_at(&self, (x, y): (f32, f32), button: PointerEventButton) {
        self.event(WindowEvent::PointerReleased { position: LogicalPosition::new(x, y), button });
    }

    pub(super) fn click(&self, caption: &str, held: &[Key]) {
        let at = self.centre(caption);
        for k in held {
            self.event(WindowEvent::KeyPressed { text: (*k).into() });
        }
        self.press_at(at, PointerEventButton::Left);
        self.release_at(at, PointerEventButton::Left);
        for k in held.iter().rev() {
            self.event(WindowEvent::KeyReleased { text: (*k).into() });
        }
    }

    pub(super) fn right_click(&self, caption: &str) {
        let at = self.centre(caption);
        self.press_at(at, PointerEventButton::Right);
        self.release_at(at, PointerEventButton::Right);
        self.popup.set(true);
    }

    /// A press carried in eight steps to `to`, as a hand drags.
    pub(super) fn drag(&self, from: (f32, f32), to: (f32, f32)) {
        self.press_at(from, PointerEventButton::Left);
        for k in 1..=8 {
            let f = k as f32 / 8.0;
            let position = LogicalPosition::new(from.0 + (to.0 - from.0) * f, from.1 + (to.1 - from.1) * f);
            self.event(WindowEvent::PointerMoved { position });
        }
        self.release_at(to, PointerEventButton::Left);
    }

    /// Lets the system's double-click interval pass, so the next click is a first one.
    pub(super) fn rest(&self) {
        let start = Instant::now();
        while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
            self.rig.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(super) fn selected(&self) -> Vec<String> {
        let mut tiles = app::tiles(self.w);
        tiles.sort_by_key(|t| t.index);
        tiles.into_iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect()
    }

    /// The sidebar's photo rows lit, in the tree's order.
    pub(super) fn lit_rows(&self) -> Vec<String> {
        let rows = self.w.global::<Library>().get_rows();
        (0..rows.row_count()).filter_map(|i| rows.row_data(i)).filter(|r| r.kind == 2 && r.selected).map(|r| r.title.to_string()).collect()
    }

    pub(super) fn shown(&self) -> Vec<String> {
        let mut tiles = app::tiles(self.w);
        tiles.sort_by_key(|t| t.index);
        tiles.into_iter().map(|t| t.caption.to_string()).collect()
    }

    pub(super) fn menu(&self) -> Vec<String> {
        let lines = self.w.global::<Library>().get_menu_lines();
        (0..lines.row_count()).filter_map(|i| lines.row_data(i)).map(|l| l.text.to_string()).collect()
    }

    pub(super) fn sub_menu(&self) -> Vec<String> {
        let lines = self.w.global::<Library>().get_menu_sub_lines();
        (0..lines.row_count()).filter_map(|i| lines.row_data(i)).map(|l| l.text.to_string()).collect()
    }

    /// The pointer resting on a line of the menu a right click at `at` dropped, past the submenu's delay.
    pub(super) fn rest_on(&self, at: (f32, f32), text: &str) {
        let lines = self.w.global::<Library>().get_menu_lines();
        let i = (0..lines.row_count()).position(|i| lines.row_data(i).unwrap().text == text).unwrap_or_else(|| panic!("{text} in {:?}", self.menu()));
        let rules = (0..=i).filter(|&k| lines.row_data(k).unwrap().rule).count() as f32;
        // `Gap.xs` above the lines, `ButtonStyle.height` a line, `Stroke.line` a rule.
        let y = at.1 + 4.0 + rules + 32.0 * (i as f32 + 0.5);
        self.event(WindowEvent::PointerMoved { position: LogicalPosition::new(at.0 + 20.0, y) });
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(400) {
            self.rig.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(super) fn choose_sub(&self, text: &str) -> bool {
        let at = self.sub_menu().iter().position(|l| l == text).unwrap_or_else(|| panic!("{text} in {:?}", self.sub_menu()));
        let kept = self.w.global::<Library>().invoke_menu_sub_chosen(at as i32);
        self.rig.pump();
        if !kept && self.popup.replace(false) {
            let corner = (1270.0, 795.0);
            self.press_at(corner, PointerEventButton::Left);
            self.release_at(corner, PointerEventButton::Left);
        }
        kept
    }

    pub(super) fn choose(&self, text: &str) -> bool {
        let at = self.menu().iter().position(|l| l == text).unwrap_or_else(|| panic!("{text} in {:?}", self.menu()));
        let kept = self.w.global::<Library>().invoke_menu_chosen(at as i32);
        self.rig.pump();
        if !kept && self.popup.replace(false) {
            // The popup closes as the line's own click would close it: a click on the inert status bar.
            let corner = (1270.0, 795.0);
            self.press_at(corner, PointerEventButton::Left);
            self.release_at(corner, PointerEventButton::Left);
        }
        kept
    }

    pub(super) fn status(&self) -> String {
        self.w.global::<Library>().get_status().to_string()
    }
}
