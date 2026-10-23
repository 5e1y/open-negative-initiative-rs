//! What the export surface shows.

use super::*;
use oni_i18n::tr;

impl Ctl {
    pub(super) fn publish_lines(&self) {
        let Some(w) = self.window() else { return };
        let lines: Vec<ExportLine> = {
            let c = self.export.borrow();
            (0..c.targets.len()).map(|i| c.line(i)).collect()
        };
        let model = self.export.borrow().model.clone();
        // A row is set only when it changed, so an unchanged preview is never handed over again.
        if model.row_count() == lines.len() {
            for (i, l) in lines.into_iter().enumerate() {
                if model.row_data(i).as_ref() != Some(&l) {
                    model.set_row_data(i, l);
                }
            }
        } else {
            model.set_vec(lines);
        }
        w.global::<Exporting>().set_lines(ModelRc::from(model));
    }

    pub(super) fn publish_line(&self, i: usize) {
        let line = {
            let c = self.export.borrow();
            (i < c.targets.len()).then(|| c.line(i))
        };
        let model = self.export.borrow().model.clone();
        if let Some(line) = line
            && i < model.row_count()
        {
            model.set_row_data(i, line);
        }
    }

    /// Every property the surface, the chip and the menus read, read before any is set.
    pub(in crate::app) fn publish_export(&self) {
        let Some(w) = self.window() else { return };
        let (selected, has_photo) = {
            let s = self.state.borrow();
            (s.picks.selection.len(), s.open.is_some())
        };
        let c = self.export.borrow();
        let e = w.global::<Exporting>();
        let space = c.settings.space().clone();
        let count = c.targets.len();
        let batch = count > 1;
        e.set_shown(c.shown);
        e.set_on_paper(space.paper().is_some());
        e.set_count(count as i32);
        e.set_subtitle(export::subtitle(&c.targets, c.exporting, c.progress, c.stopped).into());
        e.set_extension(format!(".{}", c.settings.format().file_extension()).into());
        e.set_format(Format::ALL.iter().position(|f| *f == c.settings.format()).unwrap_or(0) as i32);
        let on_profile = matches!(space, Space::Profile(_));
        e.set_space_tab(if on_profile || c.pending { 4 } else { Space::BUILT_IN.iter().position(|s| *s == space).unwrap_or(1) as i32 });
        e.set_purpose(space.purpose().into());
        e.set_jpeg(c.settings.format() == Format::Jpeg);
        e.set_on_profile(on_profile);
        e.set_pending(c.pending && !on_profile);
        e.set_pending_note(tr!("Until a profile is chosen, the picture stays in {space}.", space = space.title()).into());
        if let Space::Profile(chosen) = &space {
            let (listed, titles) = c.profile_titles(chosen);
            let at = listed.iter().position(|p| p == chosen).unwrap_or(0);
            e.set_profiles(ModelRc::new(VecModel::from(titles.into_iter().map(SharedString::from).collect::<Vec<_>>())));
            e.set_profile(at as i32);
            e.set_profile_note(export::profile_note(chosen, &c.installed).into());
        }
        e.set_colour_only(c.colour_only);
        e.set_edited_in(export::edited_in_note(c.edited_in.as_ref(), &space, batch).into());
        e.set_compress(c.settings.compress_tiff);
        e.set_quality_fraction(Ctl::quality_fraction(c.settings.jpeg_quality));
        e.set_quality_readout(format!("{:.0}%", c.settings.jpeg_quality * 100.0).into());
        let (wt, ht) = export::size_texts(c.full(), c.settings.resize);
        e.set_width_text(wt.into());
        e.set_height_text(ht.into());
        e.set_size_enabled(c.full().is_some());
        e.set_resized(c.settings.resize != Resize::Full);
        e.set_hdr(c.settings.hdr);
        e.set_bits(export::bits(c.settings.format()).into());
        let dims = c.full().map(|(fw, fh)| {
            let (ow, oh) = c.settings.resize.output_size(fw, fh);
            format!("{} × {}", ow as i64, oh as i64)
        });
        e.set_dims(dims.unwrap_or_else(|| "—".into()).into());
        e.set_weight_label(if batch { tr("estimated total size") } else { tr("estimated size") }.into());
        e.set_weight(export::weight(c.estimate, count).into());
        e.set_low_precision(export::low_precision(c.low_precision, count).into());
        e.set_exporting(c.exporting);
        e.set_can_export(count > 0);
        e.set_key_title(label::key(count).into());
        e.set_menu_title(label::menu(if c.shown { count } else { selected }).into());
        e.set_menu_live(if c.shown { count > 0 && !c.exporting } else { has_photo || selected > 0 });
        e.set_gamut_title(if c.gamut { tr("Hide What This Screen Cannot Show") } else { tr("Show What This Screen Cannot Show") }.into());
        e.set_chip_title(space.title().into());
        e.set_chip_hint(space.hint().into());
        e.set_confirm_shown(c.confirm.is_some());
        e.set_notice_shown(c.notice.is_some());
        if let Some((title, message)) = &c.notice {
            e.set_notice_title(title.as_str().into());
            e.set_notice_message(message.as_str().into());
        }
        drop(c);
        self.publish_menus();
    }
}
