//! The keys no menu item carries: Escape and Return on a sheet, the Lab's, the grid's and the walk keys.

use super::*;

impl Ctl {
    /// The keys no menu item carries: Escape and Return on a sheet, the Lab's and the walk keys, and
    /// ⌘=, which types apart from the menu's ⌘+; every other shortcut is the menu bar's.
    pub(super) fn key(self: &Rc<Self>, text: &str, command: bool, shift: bool, bare: bool, repeat: bool) -> bool {
        let Some(w) = self.window() else { return false };
        let confirm = w.global::<Confirm>();
        if confirm.get_shown() {
            // Quitting's default is Cancel, so Return never discards unsaved work; a destructive key
            // takes no Return at all, so no stray key confirms it.
            match text {
                "\u{1b}" => self.answered(false),
                "\r" | "\n" if confirm.get_cancel_default() => self.answered(false),
                "\r" | "\n" if !confirm.get_destructive() => self.answered(true),
                _ => {}
            }
            return true;
        }
        let help = w.global::<crate::Help>();
        if help.get_welcome_shown() && matches!(text, "\u{1b}" | "\r" | "\n") {
            help.invoke_welcome_closed();
            return true;
        }
        if help.get_report_shown() && text == "\u{1b}" {
            help.invoke_report_closed();
            return true;
        }
        if w.global::<crate::Stocks>().get_shown() {
            if text == "\u{1b}" {
                w.global::<crate::Stocks>().invoke_chosen(-1);
            }
            return true;
        }
        if w.global::<crate::Choice>().get_shown() {
            if text == "\u{1b}" {
                w.global::<crate::Choice>().invoke_chosen(-1);
            }
            return true;
        }
        if w.global::<crate::Advice>().get_shown() {
            if matches!(text, "\u{1b}" | "\r" | "\n") {
                self.advice_closed();
            }
            return true;
        }
        if self.mask_dialog_key(text) {
            return true;
        }
        if self.preset_form_key(text) {
            return true;
        }
        if text == "\u{1b}" && self.cancel_frame_drag() {
            return true;
        }
        if text == "\u{1b}" && w.global::<Pairing>().get_shown() {
            self.close_pairing();
            return true;
        }
        // A sheet is modal: Escape closes the ones no branch above reads, and no key reaches behind it.
        if w.get_sheet_shown() {
            match text {
                "\u{1b}" => self.escape_sheet(&w),
                "\r" | "\n" => self.return_sheet(&w),
                _ => {}
            }
            return true;
        }
        if text == "\u{1b}" && self.export_escape() {
            return true;
        }
        if self.state.borrow().prepare.shown {
            return self.prepare_key(text, command, shift, bare);
        }
        if self.lab_key(text, bare, repeat) {
            return true;
        }
        if w.global::<App>().get_gallery() && self.grid_key(text, command, shift, bare, repeat) {
            return true;
        }
        // `keyboardShortcut("+")` answers ⌘= too, the key that carries + unshifted.
        if command && !shift && text == "=" && w.global::<Menus>().get_live().larger {
            self.act("larger");
            return true;
        }
        false
    }

    /// `.defaultAction`: Return presses the one key a notice offers; the export's clash, whose key
    /// replaces files, takes none.
    fn return_sheet(self: &Rc<Self>, w: &crate::MainWindow) {
        let e = w.global::<crate::Exporting>();
        if e.get_notice_shown() {
            e.invoke_notice_closed();
        } else if w.global::<crate::Notice>().get_shown() {
            w.global::<crate::Notice>().invoke_close();
        } else if w.global::<crate::Navigator>().get_catch_up_shown() {
            w.global::<crate::Navigator>().invoke_catch_up_done();
        }
    }

    fn escape_sheet(self: &Rc<Self>, w: &crate::MainWindow) {
        let e = w.global::<crate::Exporting>();
        if e.get_confirm_shown() || e.get_notice_shown() || e.get_papers_shown() {
            self.export_escape();
        } else if w.global::<crate::Notice>().get_shown() {
            w.global::<crate::Notice>().invoke_close();
        } else if w.global::<crate::Navigator>().get_catch_up_shown() {
            w.global::<crate::Navigator>().invoke_catch_up_done();
        } else if w.global::<crate::Wrapped>().get_shown() {
            w.global::<crate::Wrapped>().invoke_close();
        }
    }
}
