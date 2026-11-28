//! Select Film Stock's sheet laid: the stocks, brand after brand, as its search and its brand filter leave them, three
//! cards a row, and the filter's brands, each with how many stocks it lists.

use super::super::Ctl;
use super::{StockTarget, find};
use crate::{StockCell, StockRow, Stocks};
use oni_settings::film::stocks;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

impl Ctl {
    /// The sheet on `current`, the stock picked marked, searched for nothing and on every brand.
    pub(crate) fn open_stock_picker(&self, target: StockTarget, current: Option<&str>) {
        {
            let mut s = self.state.borrow_mut();
            (s.stock_target, s.stock_current, s.stock_find) = (Some(target), current.map(str::to_owned), find::Find::default());
        }
        self.lay_stock_picker();
        if let Some(w) = self.window() {
            w.global::<Stocks>().set_shown(true);
        }
    }

    /// The sheet's stocks as its search and brand leave them, and the brand filter's entries with their counts.
    pub(super) fn lay_stock_picker(&self) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Stocks>();
        let columns = g.get_columns().max(1) as usize;
        let (current, found) = {
            let s = self.state.borrow();
            (s.stock_current.clone(), s.stock_find.clone())
        };
        let listed = crate::stocks::by_brand(current.as_deref());
        let all = stocks();
        let total: usize = listed.iter().map(|(_, kinds)| kinds.len()).sum();
        let mut brands: Vec<SharedString> = vec![oni_i18n::tr!("Every Brand · {n}", n = total).into()];
        brands.extend(listed.iter().map(|(brand, kinds)| SharedString::from(format!("{brand} · {}", kinds.len()))));
        let chosen = found.brand.as_deref().and_then(|b| listed.iter().position(|(brand, _)| *brand == b)).map_or(0, |i| i + 1);
        let cells: Vec<StockCell> = listed
            .into_iter()
            .flat_map(|(brand, kinds)| kinds.into_iter().map(move |i| (brand, i)))
            .map(|(brand, i)| (brand, i, all[i]))
            .filter(|(brand, _, kind)| found.shows(brand, crate::stocks::shown_name(kind)))
            .map(|(brand, i, kind)| {
                let artwork = crate::stocks::icon(kind.id);
                let note = crate::stocks::unspecified(&kind);
                StockCell {
                    index: i as i32,
                    name: crate::stocks::shown_name(&kind).into(),
                    detail: if note.is_empty() { brand.into() } else { note.into() },
                    has_artwork: artwork.is_some(),
                    artwork: artwork.unwrap_or_default(),
                    picked: current.as_deref() == Some(kind.id),
                }
            })
            .collect();
        let rows: Vec<StockRow> = cells.chunks(columns).map(|c| StockRow { cells: ModelRc::new(VecModel::from(c.to_vec())) }).collect();
        g.set_rows(ModelRc::new(VecModel::from(rows)));
        g.set_brands(ModelRc::new(VecModel::from(brands)));
        g.set_brand(chosen as i32);
    }

    /// The sheet, where it is open, laid again on the catalogue in use, its target, pick, words and brand kept.
    pub(in crate::app) fn stock_picker_again(&self) {
        let shown = self.window().is_some_and(|w| w.global::<Stocks>().get_shown());
        if shown && self.state.borrow().stock_target.is_some() {
            self.lay_stock_picker();
        }
    }
}
