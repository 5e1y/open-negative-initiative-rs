//! Entering the export surface and leaving it.

use super::*;

impl Ctl {
    /// Menu, bar and ⌘E: on the surface they confirm, everywhere else they open it.
    pub(in crate::app) fn export_act(self: &Rc<Self>) {
        if self.export.borrow().shown {
            self.run_export();
        } else {
            self.export_now();
        }
    }

    /// `screenGamutWarning` kept for the next launch, and the View menu's word for it.
    pub(in crate::app) fn gamut_kept(&self) {
        let on = self.state.borrow().screen_gamut;
        {
            let mut c = self.export.borrow_mut();
            c.gamut = on;
            c.save(SCREEN_GAMUT, Value::from(on));
        }
        self.publish_export();
    }

    /// `exportNow`: the selection in gallery order, else beside it the open photo.
    pub fn export_now(self: &Rc<Self>) {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        let targets = {
            let s = self.state.borrow();
            let paths = export::targets(&s.order, &s.selection(), s.open.as_deref().filter(|_| !gallery));
            let open = s.open_path().map(str::to_string);
            let current = s.store.settings().clone();
            paths
                .into_iter()
                .map(|p| {
                    let settings = if Some(&p) == open.as_ref() {
                        current.clone()
                    } else {
                        let mosaic = oni_decode::decodes_via_mosaic(Path::new(&p));
                        oni_settings::sidecar::read(Path::new(&p), mosaic, true).map(|l| l.settings).unwrap_or_default()
                    };
                    Target { path: PathBuf::from(p), settings }
                })
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            return;
        }
        self.flush();
        self.enter_export(targets);
    }

    /// `enterExport`: entering from the surface keeps the earlier return, or Cancel would land on it.
    pub fn enter_export(self: &Rc<Self>, targets: Vec<Target>) {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        {
            let mut c = self.export.borrow_mut();
            if !c.shown {
                c.return_gallery = gallery;
                c.edited_in = Some(c.settings.space().clone());
            }
            c.shown = true;
            c.stopped = None;
            c.stop.store(false, Ordering::Relaxed);
            c.progress = 0;
            c.rows = vec![None; targets.len()];
            c.sizes = vec![None; targets.len()];
            c.previews = vec![Preview::Waiting; targets.len()];
            c.low_precision = 0;
            c.estimate = None;
            c.targets = targets;
        }
        self.publish_lines();
        self.publish_export();
        self.read_targets();
    }

    /// `leaveExport`: back where the surface was opened from.
    pub fn leave_export(self: &Rc<Self>) {
        let back = {
            let mut c = self.export.borrow_mut();
            if !c.shown {
                return;
            }
            c.shown = false;
            c.stopped = None;
            // The column goes with the card; the next one lays out its own room.
            if let Ok(mut room) = c.room.lock() {
                *room = None;
            }
            c.pass.fetch_add(1, Ordering::Relaxed);
            c.weighing.fetch_add(1, Ordering::Relaxed);
            c.return_gallery
        };
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(back);
        }
        self.publish_export();
        self.publish_bar();
        self.publish_tiles();
        self.request_render();
    }

    pub(super) fn cancel_export(self: &Rc<Self>) {
        let exporting = self.export.borrow().exporting;
        if exporting {
            self.export.borrow().stop.store(true, Ordering::Relaxed);
        } else {
            self.leave_export();
        }
    }

    /// Esc on the surface is Cancel, or Stop mid-batch.
    pub(in crate::app) fn export_escape(self: &Rc<Self>) -> bool {
        let (shown, sheet) = {
            let c = self.export.borrow();
            (c.shown, c.confirm.is_some() || c.notice.is_some() || c.film_row.is_some() || c.papers.shown)
        };
        if sheet {
            self.close_export_sheets();
            return true;
        }
        if shown {
            self.cancel_export();
        }
        shown
    }

    pub(super) fn close_export_sheets(self: &Rc<Self>) {
        {
            let mut c = self.export.borrow_mut();
            c.confirm = None;
            c.notice = None;
            c.film_row = None;
            c.papers.shown = false;
        }
        self.publish_export();
        self.publish_papers();
    }
}
