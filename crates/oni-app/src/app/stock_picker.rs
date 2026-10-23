//! `StockPickerSheet` in the window: Select Roll Type for a roll, and the export's film field for one
//! frame, the stocks grouped by brand; a click picks and closes, and Done leaves things as they were.

use super::Ctl;
use crate::{Exporting, StockBrand, StockCell, StockRow, Stocks};
use oni_i18n::tr;
use oni_settings::film::FILM_STOCKS;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

/// What a pick lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum StockTarget {
    /// The roll the info sidebar shows: `setFilmStock`.
    Roll,
    /// The export row whose film field opened it: this frame alone.
    Export,
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
        let brands: Vec<StockBrand> = crate::stocks::by_brand()
            .into_iter()
            .map(|(brand, kinds)| {
                let cells: Vec<StockCell> = kinds
                    .into_iter()
                    .map(|i| {
                        let kind = &FILM_STOCKS[i];
                        let artwork = crate::stocks::icon(kind.icon_name);
                        StockCell {
                            index: i as i32,
                            name: kind.display_name.into(),
                            has_artwork: artwork.is_some(),
                            artwork: artwork.unwrap_or_default(),
                            picked: current == Some(kind.id),
                        }
                    })
                    .collect();
                let rows: Vec<StockRow> = cells.chunks(columns).map(|c| StockRow { cells: ModelRc::new(VecModel::from(c.to_vec())) }).collect();
                StockBrand { brand: brand.to_uppercase().into(), rows: ModelRc::new(VecModel::from(rows)) }
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
            (Some(StockTarget::Roll), Some(kind)) => self.update_roll(tr("Set Film Stock"), |f| f.stock = Some(kind.id.to_string())),
            (Some(StockTarget::Export), _) => w.global::<Exporting>().invoke_stock_chosen(i.map_or(-1, |i| i as i32)),
            _ => {}
        }
    }

    /// `showingStockPicker` from the info sidebar: the roll's own stock marked.
    pub(super) fn select_roll_type(&self) {
        let current = self.state.borrow().roll_stock();
        self.open_stock_picker(StockTarget::Roll, current.as_deref());
    }
}
