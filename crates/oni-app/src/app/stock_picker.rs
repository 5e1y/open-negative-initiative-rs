//! `StockPickerSheet` in the window: Select Film Stock for a roll and the export's film field for one frame, the
//! stocks grouped by brand, a roll's line shown among its stocks; a click picks and closes, and Done leaves things
//! as they were. The editor's film and the Colour tab's paper drop a menu instead, each stock or paper it lights shown
//! on the open photo unwritten.

use super::Ctl;
use crate::{Exporting, PanelState, StockBrand, StockCell, StockLine, StockRow, Stocks};
use oni_settings::film::{FilmStockKind, film_stock, stocks};
use oni_settings::{PaperStock, PipelineSettings};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

/// What an editor's menu lights on the open photo, nothing written: a stock by its place in the catalogue, or a paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Lit {
    Stock(usize),
    Paper(PaperStock),
}

impl Lit {
    /// What its pick writes on `settings`, which the menu shows unwritten while it lights it; `own`, the stock in use.
    /// A paper changes the one the enlarger prints on, never turns the enlarger on.
    pub(super) fn picked(self, settings: &PipelineSettings, own: Option<&str>) -> PipelineSettings {
        match self {
            Lit::Stock(i) => stocks().get(i).map_or_else(|| settings.clone(), |kind| picked(settings, kind, own)),
            Lit::Paper(paper) if settings.print_paper.is_some() => PipelineSettings { print_paper: Some(paper), ..settings.clone() },
            Lit::Paper(_) => settings.clone(),
        }
    }

    /// The paper at `i` in the catalogue, -1 none.
    fn paper(i: i32) -> Option<Lit> {
        usize::try_from(i).ok().and_then(|i| PaperStock::all().get(i).copied()).map(Lit::Paper)
    }

    fn stock(i: i32) -> Option<Lit> {
        usize::try_from(i).ok().map(Lit::Stock)
    }
}

/// What a pick of `kind` writes on the open photo, which the menu shows unwritten while it lights it: a digital
/// photo shot on that virtual film, a roll's photo on the stock's measured dyes; `own`, the stock in use, nothing.
pub(super) fn picked(settings: &PipelineSettings, kind: &FilmStockKind, own: Option<&str>) -> PipelineSettings {
    if own == Some(kind.id) {
        return settings.clone();
    }
    match settings.film() {
        Some(_) => crate::stocks::virtual_stock(kind.id).map_or_else(|| settings.clone(), |v| super::roll_tab::picked_film(settings, v)),
        None => {
            let mut out = settings.clone();
            super::organising::roll_card::dyed(&mut out, Some(kind.id));
            out
        }
    }
}

/// What a pick lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum StockTarget {
    /// The roll the info sidebar shows: `setFilmStock`.
    Roll,
    /// The export row whose film field opened it: this frame alone.
    Export,
    /// Prepare the Roll's Film step: the roll it prepares, written with its measure.
    Prepare,
}

impl Ctl {
    pub(super) fn wire_stock_picker(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let g = window.global::<Stocks>();
        let p = me.clone();
        g.on_chosen(move |i| p.stock_chosen(usize::try_from(i).ok()));
        let p = me.clone();
        g.on_dropping(move || p.lay_stock_menu());
        let p = me.clone();
        g.on_lit(move |i| p.light(Lit::stock(i)));
        let p = me.clone();
        g.on_picked(move |i| p.menu_picked(Lit::stock(i)));
        let panel = window.global::<PanelState>();
        let p = me.clone();
        panel.on_paper_dropping(move || p.lay_paper_menu());
        let p = me.clone();
        panel.on_paper_lit(move |i| p.light(Lit::paper(i)));
        let p = me.clone();
        panel.on_paper_chosen(move |i| p.menu_picked(Lit::paper(i)));
    }

    /// The editor's menu on the open photo's film: a digital photo's virtual films alone, else every stock a roll
    /// takes, its line shown among them; each with its canister at a tree row's side, the stock in use checked.
    pub(super) fn lay_stock_menu(&self) {
        let Some(w) = self.window() else { return };
        let (film, own) = {
            let s = self.state.borrow();
            (s.store.settings().film().is_some(), s.open_stock().map(|k| k.id))
        };
        let side = crate::stocks::row_side(&w);
        let listed = if film { crate::stocks::virtual_by_brand() } else { crate::stocks::by_brand(own) };
        let all = stocks();
        let mut lines = Vec::new();
        for (brand, kinds) in listed {
            lines.push(StockLine { index: -1, name: brand.into(), brand: true, ..Default::default() });
            for i in kinds {
                let kind = all[i];
                let artwork = crate::stocks::row_icon(kind.id, side);
                lines.push(StockLine { index: i as i32, name: crate::stocks::shown_name(&kind).into(), has_artwork: artwork.is_some(), artwork: artwork.unwrap_or_default(), brand: false });
            }
        }
        let in_use = lines.iter().position(|l| !l.brand && Some(all[l.index as usize].id) == own).map_or(-1, |i| i as i32);
        let g = w.global::<Stocks>();
        g.set_lines(ModelRc::new(VecModel::from(lines)));
        g.set_in_use(in_use);
        self.state.borrow_mut().menu_baked = false;
    }

    /// The Colour tab's menu on the open photo's paper: every paper of the catalogue by maker, the one in use checked.
    pub(super) fn lay_paper_menu(&self) {
        let Some(w) = self.window() else { return };
        let own = self.state.borrow().store.settings().fitting_paper();
        let papers = PaperStock::all();
        let mut lines = Vec::new();
        for (maker, listed) in crate::stocks::papers_by_maker() {
            lines.push(StockLine { index: -1, name: maker.into(), brand: true, ..Default::default() });
            lines.extend(listed.into_iter().map(|i| StockLine { index: i as i32, name: papers[i].name().into(), ..Default::default() }));
        }
        let in_use = lines.iter().position(|l| !l.brand && papers[l.index as usize] == own).map_or(-1, |i| i as i32);
        let panel = w.global::<PanelState>();
        panel.set_paper_lines(ModelRc::new(VecModel::from(lines)));
        panel.set_paper_in_use(in_use);
        self.state.borrow_mut().menu_baked = false;
    }

    /// The stock or paper a menu lights shown on the open photo, nothing written; `None`, its own again. A light
    /// that changes nothing the photo shows, as a stock naming no dyes on a negative, asks no render.
    fn light(self: &Rc<Self>, lit: Option<Lit>) {
        let moved = {
            let mut s = self.state.borrow_mut();
            if s.lit == lit {
                return;
            }
            let before = s.live_settings();
            s.lit = lit;
            let moved = s.live_settings() != before;
            if moved {
                s.ask_view_again();
            }
            moved
        };
        if let Some(lit) = lit {
            self.bake_menu(lit);
        }
        if moved {
            self.request_render();
        }
    }

    /// The tables a render on each line of `lit`'s menu reads, baked off this thread at its first light, so the
    /// next line lit lands without its bake; a table already baked costs nothing.
    fn bake_menu(&self, lit: Lit) {
        let settings: Vec<PipelineSettings> = {
            let mut s = self.state.borrow_mut();
            if std::mem::replace(&mut s.menu_baked, true) {
                return;
            }
            let own = s.open_stock().map(|k| k.id);
            let base = s.store.settings().clone();
            let listed: Vec<Lit> = match lit {
                Lit::Stock(_) => {
                    let listed = if base.film().is_some() { crate::stocks::virtual_by_brand() } else { crate::stocks::by_brand(own) };
                    listed.into_iter().flat_map(|(_, l)| l).map(Lit::Stock).collect()
                }
                Lit::Paper(_) => PaperStock::all().into_iter().map(Lit::Paper).collect(),
            };
            listed.into_iter().map(|l| l.picked(&base, own)).collect()
        };
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            for s in &settings {
                oni_pipeline::bake_ahead(s);
            }
        });
    }

    /// A stock or paper chosen in its menu, written as one step: a digital photo shot on the stock, else the open
    /// photo's roll on it for every photo of the roll; the photo printed on the paper; what is in use writes nothing.
    /// The light it ends showed what is written, so the canvas renders nothing again.
    fn menu_picked(self: &Rc<Self>, lit: Option<Lit>) {
        let (film, own, open) = {
            let mut s = self.state.borrow_mut();
            s.lit = None;
            (s.store.settings().film().is_some(), s.open_stock().map(|k| k.id), s.open_path().map(str::to_string))
        };
        match lit {
            Some(Lit::Stock(i)) if !film => {
                if let (Some(kind), Some(path)) = (stocks().get(i).copied(), open) {
                    self.set_stock_of_roll_at(&path, kind.id);
                }
            }
            Some(lit) => self.write_settings(|s| *s = lit.picked(s, own)),
            None => {}
        }
        self.request_render();
    }

    /// The sheet on `current`, the stock picked marked.
    pub(crate) fn open_stock_picker(&self, target: StockTarget, current: Option<&str>) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Stocks>();
        let columns = g.get_columns().max(1) as usize;
        let listed = crate::stocks::by_brand(current);
        let all = stocks();
        let brands: Vec<StockBrand> = listed
            .into_iter()
            .map(|(brand, kinds)| {
                let cells: Vec<StockCell> = kinds
                    .into_iter()
                    .map(|i| {
                        let kind = all[i];
                        let artwork = crate::stocks::icon(kind.id);
                        StockCell {
                            index: i as i32,
                            name: crate::stocks::shown_name(&kind).into(),
                            note: crate::stocks::unspecified(&kind).into(),
                            has_artwork: artwork.is_some(),
                            artwork: artwork.unwrap_or_default(),
                            picked: current == Some(kind.id),
                        }
                    })
                    .collect();
                let rows: Vec<StockRow> = cells.chunks(columns).map(|c| StockRow { cells: ModelRc::new(VecModel::from(c.to_vec())) }).collect();
                StockBrand { brand: brand.into(), rows: ModelRc::new(VecModel::from(rows)) }
            })
            .collect();
        g.set_brands(ModelRc::new(VecModel::from(brands)));
        let mut s = self.state.borrow_mut();
        (s.stock_target, s.stock_current) = (Some(target), current.map(str::to_owned));
        g.set_shown(true);
    }

    /// The sheet, where it is open, laid again on the catalogue in use, its target and pick kept.
    pub(super) fn stock_picker_again(&self) {
        let Some(w) = self.window() else { return };
        let (target, current) = {
            let s = self.state.borrow();
            (s.stock_target, s.stock_current.clone())
        };
        if let (true, Some(target)) = (w.global::<Stocks>().get_shown(), target) {
            self.open_stock_picker(target, current.as_deref());
        }
    }

    /// Never a stock taken away: a roll was shot on some film, so Done only closes.
    fn stock_chosen(self: &Rc<Self>, i: Option<usize>) {
        let Some(w) = self.window() else { return };
        w.global::<Stocks>().set_shown(false);
        w.invoke_refocus();
        let target = self.state.borrow_mut().stock_target.take();
        let kind = i.and_then(|i| stocks().get(i).copied());
        match (target, kind) {
            (Some(StockTarget::Roll), Some(kind)) => self.set_roll_stock(kind.id),
            (Some(StockTarget::Export), _) => w.global::<Exporting>().invoke_stock_chosen(i.map_or(-1, |i| i as i32)),
            (Some(StockTarget::Prepare), Some(kind)) => {
                self.state.borrow_mut().prepare.stock = Some(kind.id.to_string());
                self.save_draft();
                self.publish_prepare();
            }
            _ => {}
        }
    }

    /// `showingStockPicker` from the info sidebar: the roll's own stock marked.
    pub(super) fn select_roll_type(&self) {
        let current = self.state.borrow().roll_stock();
        self.open_stock_picker(StockTarget::Roll, current.as_deref());
    }

    /// The open photo's film as the panel's hero shows it: a digital photo's virtual film, else its roll's stock.
    pub(super) fn open_canister(&self) -> crate::Canister {
        let s = self.state.borrow();
        let settings = s.store.settings();
        match settings.film() {
            Some(f) if !f.stock.is_known() => crate::stocks::missing_canister(f.stock.key()),
            Some(f) => crate::stocks::canister(film_stock(Some(f.stock.catalogue_id())), String::new()),
            None => super::organising::info_sidebar::roll_canister(s.open_stock(), Some(settings.mode)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::{ConversionMode, DyeSet, VirtualStock};

    fn kind(id: &str) -> FilmStockKind {
        film_stock(Some(id)).unwrap()
    }

    /// A digital photo lit on another virtual film is that film's reference; its own film, or a stock no
    /// digital photo is shot on, leaves it as it is.
    #[test]
    fn a_digital_photo_lit_on_a_stock_is_what_its_pick_writes() {
        let digital = PipelineSettings { mode: ConversionMode::Digital, ..PipelineSettings::default() };
        let own = digital.film().unwrap().stock;
        let next = VirtualStock::all().iter().copied().find(|v| *v != own).unwrap();
        let lit = picked(&digital, &kind(next.catalogue_id()), Some(own.catalogue_id()));
        assert_eq!(lit, roll_tab_pick(&digital, next));
        assert_eq!((lit.film().map(|f| f.stock), lit.gains.stops, lit.dye_set), (Some(next), next.reference().stops, next.dye_set()));
        assert_eq!(picked(&digital, &kind(own.catalogue_id()), Some(own.catalogue_id())), digital, "its own film writes nothing");
        assert_eq!(picked(&digital, &kind("agfaphoto-apx"), Some(own.catalogue_id())), digital, "no virtual film, nothing");
        // Twin: lit on another film, the photo is no longer as it was.
        assert_ne!(lit, digital);
    }

    /// A roll's photo lit on a stock takes the dyes the stock names; its own stock or one naming none, nothing.
    #[test]
    fn a_negative_lit_on_a_stock_takes_its_dyes() {
        let negative = PipelineSettings::default();
        assert!(negative.film().is_none());
        let lit = picked(&negative, &kind("fujifilm-c200"), None);
        assert_eq!(lit.dye_set, DyeSet::of("fujifilm-c200"));
        assert_eq!(PipelineSettings { dye_set: negative.dye_set, ..lit.clone() }, negative, "its dyes alone move");
        assert_eq!(picked(&negative, &kind("fujifilm-c200"), Some("fujifilm-c200")), negative, "the roll's own stock, nothing");
        assert_eq!(picked(&negative, &kind("agfaphoto-apx"), None), negative, "a stock naming no dyes, nothing");
        // Twin: the dyes did move from the photo's own.
        assert_ne!(negative.dye_set, DyeSet::of("fujifilm-c200"));
    }

    /// A photo lit on a paper prints on it and moves nothing else; lit on its own paper, it is as it was.
    #[test]
    fn a_photo_lit_on_a_paper_is_what_its_pick_writes() {
        let digital = PipelineSettings { mode: ConversionMode::Digital, ..PipelineSettings::default() }.on_paper();
        let own = digital.paper().unwrap();
        let other = PaperStock::all().into_iter().find(|p| *p != own).unwrap();
        let lit = Lit::Paper(other).picked(&digital, None);
        assert_eq!(lit, PipelineSettings { print_paper: Some(other), ..digital.clone() });
        assert_eq!(Lit::Paper(own).picked(&digital, None), digital, "its own paper writes nothing");
        let unprinted = PipelineSettings { print_paper: None, ..digital.clone() };
        assert_eq!(Lit::Paper(other).picked(&unprinted, None), unprinted, "the enlarger off stays off");
        let at = PaperStock::all().iter().position(|p| *p == other).unwrap() as i32;
        assert_eq!((Lit::paper(at), Lit::paper(-1), Lit::paper(PaperStock::all().len() as i32)), (Some(Lit::Paper(other)), None, None));
        // Twin: lit on another paper, the photo is no longer as it was.
        assert_ne!(lit, digital);
    }

    fn roll_tab_pick(s: &PipelineSettings, v: VirtualStock) -> PipelineSettings {
        super::super::roll_tab::picked_film(s, v)
    }
}
