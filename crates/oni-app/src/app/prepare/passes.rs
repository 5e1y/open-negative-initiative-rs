//! RGB Passes in Prepare the Roll: the roll's files grouped into triplets by their pass order, the
//! order read off the first frame's files on a click, Shift by One File, and the panel's words.

use super::{Capture, Frame, Roll, picture};
use crate::app::{Ctl, with_shell};
use crate::passes::{self, PassRead};
use crate::Preparing;
use oni_library::rgb_passes::{self, Grouping};
use oni_settings::Date;
use oni_settings::frame::PassOrder;
use slint::{Image, ModelRc, SharedString, VecModel};
use std::collections::HashMap;
use std::rc::Rc;

/// What the index knows of a file: its capture time and whether it leads passes already.
pub(super) type Known = HashMap<String, (Option<Date>, bool)>;

/// The count under the Pass Order.
pub fn grouping_note(g: &Grouping) -> String {
    format!("{} frames from {} files", g.frames.len(), g.frames.len() * 3 + g.left_over.len())
}

/// `groupingRemark`: the files left out and the stretched frames, empty when there are none.
pub fn grouping_remark(g: &Grouping) -> String {
    let mut parts = Vec::new();
    if !g.left_over.is_empty() {
        let n = g.left_over.len();
        parts.push(format!("{n} file{} left out, the files not falling into threes.", if n == 1 { " is" } else { "s are" }));
    }
    if !g.stretched.is_empty() {
        let n = g.stretched.len();
        parts.push(format!("{n} frame{} passes lie further apart than the frames do, which a missing or extra pass causes.", if n == 1 { "'s" } else { "s'" }));
    }
    if parts.is_empty() {
        return String::new();
    }
    parts.join(" ") + " Shift the grouping by one file, or leave a stray file out of the selection before preparing the roll."
}

/// `DSSlots.moved` on the order's bands: the band at `from` dropped at `to`, `None` past the slots.
pub fn moved(order: PassOrder, from: usize, to: usize) -> Option<PassOrder> {
    let mut bands = order.channels().to_vec();
    if from >= bands.len() || to >= bands.len() {
        return None;
    }
    let band = bands.remove(from);
    bands.insert(to, band);
    PassOrder::from_channels(&bands)
}

impl Roll {
    /// `regroupRoll`: on RGB Passes, the files grouped from the offset and held, the frames their
    /// leads; else, or once every file leads passes, the files as they came. True when they changed.
    pub(super) fn regroup(&mut self, known: &Known) -> bool {
        if !self.shown || self.progress.is_some() {
            return false;
        }
        let grouped = !self.sources.is_empty() && self.sources.iter().all(|f| known.get(&f.path).is_some_and(|k| k.1));
        if self.capture != Capture::RgbPasses || grouped {
            rgb_passes::release();
            self.grouping = None;
            if self.frames.iter().map(|f| &f.path).ne(self.sources.iter().map(|f| &f.path)) {
                self.frames = self.sources.clone();
                return true;
            }
            return false;
        }
        let files: Vec<(String, Option<Date>)> = self.sources.iter().map(|f| (f.path.clone(), known.get(&f.path).and_then(|k| k.0))).collect();
        let grouping = rgb_passes::grouped(&files, self.pass_order, self.pass_offset);
        let mut held = HashMap::new();
        for g in &grouping.frames {
            held.entry(g.lead.clone()).or_insert_with(|| g.passes.clone());
        }
        rgb_passes::hold(held);
        let sources: HashMap<&String, &Frame> = self.sources.iter().map(|f| (&f.path, f)).collect();
        self.frames = grouping
            .frames
            .iter()
            .filter_map(|g| {
                let mut lead = (*sources.get(&g.lead)?).clone();
                lead.passes = Some(g.passes.clone());
                Some(lead)
            })
            .collect();
        self.grouping = Some(grouping);
        self.at = 0;
        true
    }

    /// The first frame's three files, slot by slot: the order's bands pick them out of the triplet.
    fn first_files(&self) -> Option<Vec<String>> {
        let g = self.grouping.as_ref()?.frames.first()?;
        let folder = oni_library::paths::parent(&g.lead);
        Some(self.pass_order.channels().iter().map(|c| oni_library::paths::join(&folder, g.passes.name(*c))).collect())
    }
}

impl Ctl {
    fn known(&self) -> Known {
        self.state.borrow().rows.iter().map(|e| (e.path.clone(), (e.captured_at, e.leads_passes))).collect()
    }

    /// The roll regrouped, its previews asked again for the frames it now has.
    pub(super) fn regroup_roll(self: &Rc<Self>) {
        let known = self.known();
        let mut s = self.state.borrow_mut();
        let r = &mut s.prepare;
        if r.regroup(&known) {
            r.refresh_previews();
        }
    }

    /// `rollCaptureBinding`: a new capture regroups, and RGB Passes reads the first frame's files.
    pub(super) fn capture_picked(self: &Rc<Self>, capture: Capture) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            if r.capture == capture {
                return;
            }
            r.capture = capture;
        }
        self.regroup_roll();
        if capture == Capture::RgbPasses {
            self.read_passes();
        }
        self.publish_prepare();
    }

    /// `rollPassOrderBinding`: the hand's order regroups the roll.
    pub(super) fn pass_moved(self: &Rc<Self>, from: usize, to: usize) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            match moved(r.pass_order, from, to) {
                Some(now) if now != r.pass_order => r.pass_order = now,
                _ => return,
            }
        }
        self.regroup_roll();
        self.publish_prepare();
    }

    /// `onShiftGrouping`: the triplets start one file later, three shifts coming back.
    pub(super) fn shift_grouping(self: &Rc<Self>) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            r.pass_offset = (r.pass_offset + 1) % 3;
        }
        self.regroup_roll();
        self.read_passes();
        self.publish_prepare();
    }

    /// `readRollPasses`: the first frame's three files decoded alone off this thread; their pictures
    /// go on the chips and, the order untouched meanwhile, the order their light fits replaces it.
    pub(super) fn read_passes(self: &Rc<Self>) {
        let (files, asked, ticket) = {
            let r = &mut self.state.borrow_mut().prepare;
            let Some(files) = r.first_files() else { return };
            r.passes_asked += 1;
            (files, r.pass_order, r.passes_asked)
        };
        std::thread::spawn(move || {
            let reads: Option<Vec<PassRead>> = files.iter().map(|f| passes::read(f)).collect();
            with_shell(move |c| c.passes_read(&files, asked, ticket, reads));
        });
    }

    fn passes_read(self: &Rc<Self>, files: &[String], asked: PassOrder, ticket: u64, reads: Option<Vec<PassRead>>) {
        let fitted = {
            let r = &mut self.state.borrow_mut().prepare;
            let current = r.shown && r.passes_asked == ticket && r.capture == Capture::RgbPasses && r.first_files().as_deref() == Some(files);
            let Some(reads) = reads.filter(|_| current) else { return };
            r.pass_pictures = reads.iter().map(|p| (p.path.clone(), picture(&p.picture))).collect();
            let shares: Vec<[f64; 3]> = reads.iter().map(|p| p.share).collect();
            match PassOrder::fitting(&shares) {
                Some(order) if r.pass_order == asked && order != asked => {
                    r.pass_order = order;
                    true
                }
                _ => false,
            }
        };
        if fitted {
            self.regroup_roll();
        }
        self.publish_prepare();
    }

    /// The Pass Order's chips, the count and the remark, read by `publish_prepare`.
    pub(super) fn publish_passes(&self, p: &Preparing) {
        let r = &self.state.borrow().prepare;
        let grouping = r.grouping.as_ref().filter(|_| r.capture == Capture::RgbPasses);
        p.set_grouped(grouping.is_some());
        let Some(g) = grouping else { return };
        let files = r.first_files().unwrap_or_default();
        let bands = r.pass_order.channels();
        p.set_pass_letters(ModelRc::new(VecModel::from(bands.iter().map(|c| SharedString::from(c.letter())).collect::<Vec<_>>())));
        p.set_pass_bands(ModelRc::new(VecModel::from(bands.iter().map(|c| c.index() as i32).collect::<Vec<_>>())));
        p.set_pass_pictures(ModelRc::new(VecModel::from(files.iter().map(|f| r.pass_pictures.get(f).cloned().unwrap_or_default()).collect::<Vec<Image>>())));
        p.set_pass_names(ModelRc::new(VecModel::from(files.iter().map(|f| SharedString::from(oni_library::paths::name(f))).collect::<Vec<_>>())));
        p.set_grouping_note(grouping_note(g).into());
        p.set_grouping_remark(grouping_remark(g).into());
    }

    /// `landRoll`'s flags: each lead marked as leading its passes and each pass as under it, so the
    /// gallery hides them at once, the open lead's own sidecar still on its way.
    pub(super) fn index_passes(self: &Rc<Self>, written: &super::Written, passes: &[String]) {
        {
            let s = self.state.borrow();
            if let Some(index) = &s.index {
                for (lead, _, now) in written {
                    let Some(p) = &now.rgb_passes else { continue };
                    let _ = index.set_passes(None, true, lead);
                    let name = oni_library::paths::name(lead);
                    for pass in rgb_passes::paths_beside(p, lead).iter().filter(|f| *f != lead) {
                        let _ = index.set_passes(Some(&name), false, pass);
                    }
                }
            }
        }
        rgb_passes::release();
        self.reread(passes);
    }

    pub(super) fn wire_passes(me: &Rc<Ctl>, p: &Preparing) {
        let c = me.clone();
        p.on_pass_moved(move |from, to| c.pass_moved(usize::try_from(from).unwrap_or(usize::MAX), usize::try_from(to).unwrap_or(usize::MAX)));
        let c = me.clone();
        p.on_shift_grouping(move || c.shift_grouping());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn a_band_dropped_between_two_others_slides_them_aside() {
        assert_eq!(moved(PassOrder::Bgr, 0, 2), Some(PassOrder::Grb));
        assert_eq!(moved(PassOrder::Bgr, 2, 0), Some(PassOrder::Rbg));
        assert_eq!(moved(PassOrder::Bgr, 1, 1), Some(PassOrder::Bgr));
        // Twin: a drop is a move, never a swap, which would read R G B.
        assert_ne!(moved(PassOrder::Bgr, 0, 2), Some(PassOrder::Rgb));
        assert_eq!(moved(PassOrder::Bgr, 0, 3), None);
    }

    #[test]
    fn the_grouping_says_its_count_and_what_to_check() {
        let mut g = Grouping { frames: Vec::new(), left_over: vec!["/a".into()], stretched: HashSet::new() };
        assert_eq!(grouping_note(&g), "0 frames from 1 files");
        assert_eq!(grouping_remark(&g), "1 file is left out, the files not falling into threes. Shift the grouping by one file, or leave a stray file out of the selection before preparing the roll.");
        g.left_over.push("/b".into());
        g.stretched.insert("/c".into());
        assert_eq!(
            grouping_remark(&g),
            "2 files are left out, the files not falling into threes. 1 frame's passes lie further apart than the frames do, which a missing or extra pass causes. Shift the grouping by one file, or leave a stray file out of the selection before preparing the roll."
        );
        // Twin: a clean grouping says nothing.
        assert_eq!(grouping_remark(&Grouping::default()), "");
    }
}
