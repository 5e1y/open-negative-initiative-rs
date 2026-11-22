//! `StockPickerSheet` in the window: Select Film Stock for a roll, a digital photo's virtual film and the export's
//! film field for one frame, the stocks grouped by brand, a roll's line shown among its stocks; a click picks and
//! closes, and Done leaves things as they were.

use super::Ctl;
use crate::{Exporting, StockBrand, StockCell, StockRow, Stocks};
use oni_settings::film::{FILM_STOCKS, film_stock};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

/// What a pick lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum StockTarget {
    /// The roll the info sidebar shows: `setFilmStock`.
    Roll,
    /// The export row whose film field opened it: this frame alone.
    Export,
    /// Prepare the Roll's Film step: the roll it prepares, written with its measure.
    Prepare,
    /// The editor's Roll tab: the roll the open photo lies in, one undoable step for every photo of it.
    OpenRoll,
    /// The editor's Film block: the virtual film the open digital photo is shot on, its stocks alone listed.
    Film,
}

impl Ctl {
    pub(super) fn wire_stock_picker(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let p = me.clone();
        window.global::<Stocks>().on_chosen(move |i| p.stock_chosen(usize::try_from(i).ok()));
    }

    /// The sheet on `current`, the stock picked marked.
    pub(crate) fn open_stock_picker(&self, target: StockTarget, current: Option<&str>) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Stocks>();
        let columns = g.get_columns().max(1) as usize;
        let listed = if target == StockTarget::Film { crate::stocks::virtual_by_brand() } else { crate::stocks::by_brand(current) };
        let brands: Vec<StockBrand> = listed
            .into_iter()
            .map(|(brand, kinds)| {
                let cells: Vec<StockCell> = kinds
                    .into_iter()
                    .map(|i| {
                        let kind = &FILM_STOCKS[i];
                        let artwork = crate::stocks::icon(kind.id);
                        StockCell {
                            index: i as i32,
                            name: kind.display_name.into(),
                            note: crate::stocks::unspecified(kind).into(),
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
        self.state.borrow_mut().stock_target = Some(target);
        g.set_shown(true);
    }

    /// Never a stock taken away: a roll was shot on some film, so Done only closes.
    fn stock_chosen(self: &Rc<Self>, i: Option<usize>) {
        let Some(w) = self.window() else { return };
        w.global::<Stocks>().set_shown(false);
        w.invoke_refocus();
        let target = self.state.borrow_mut().stock_target.take();
        let kind = i.and_then(|i| FILM_STOCKS.get(i));
        match (target, kind) {
            (Some(StockTarget::Roll), Some(kind)) => self.set_roll_stock(kind.id),
            (Some(StockTarget::Export), _) => w.global::<Exporting>().invoke_stock_chosen(i.map_or(-1, |i| i as i32)),
            (Some(StockTarget::OpenRoll), Some(kind)) => {
                let open = self.state.borrow().open_path().map(str::to_string);
                if let Some(path) = open {
                    self.set_stock_of_roll_at(&path, kind.id);
                }
            }
            (Some(StockTarget::Film), Some(kind)) => {
                if let Some(stock) = crate::stocks::virtual_stock(kind.id) {
                    self.write_settings(|s| *s = super::roll_tab::picked_film(s, stock));
                }
            }
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

    /// The editor's Select Film Stock: the sheet on the open digital photo's virtual film, else on its roll's stock.
    pub(super) fn select_open_stock(&self) {
        let (film, stock) = {
            let s = self.state.borrow();
            (s.store.settings().film(), s.open_stock().map(|k| k.id))
        };
        match film {
            Some(f) => self.open_stock_picker(StockTarget::Film, Some(f.stock.catalogue_id())),
            None => self.open_stock_picker(StockTarget::OpenRoll, stock),
        }
    }

    /// The open photo's film as the panel's hero shows it: a digital photo's virtual film, else its roll's stock.
    pub(super) fn open_canister(&self) -> crate::Canister {
        let s = self.state.borrow();
        let settings = s.store.settings();
        match settings.film() {
            Some(f) => crate::stocks::canister(film_stock(Some(f.stock.catalogue_id())), String::new()),
            None => super::organising::info_sidebar::roll_canister(s.open_stock(), Some(settings.mode)),
        }
    }
}
