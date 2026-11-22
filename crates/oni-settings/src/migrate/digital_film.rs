//! A digital photo a sidecar before format 38 kept on its own positive path, or on a film whose reference it
//! stored: it opens shot on its film and read by the negative's pipeline, every edit with an equivalent kept.
//! Its white, its picture's own levels and its Black and White switch have none and are dropped.

use super::chain::DIGITAL_ON_FILM_VERSION;
use super::datasheet;
use crate::PipelineSettings;
use crate::axis::density_axis;
use crate::blocks::{chroma_decompression, separation};
use crate::virtual_film::{StockReference, VirtualStock};
use serde_json::Value;

/// Portra 800's reference as format 37 derived it, for its print resting at no exposure: a filmed photo's
/// hand moves are read off it.
pub const FORMER_PORTRA_800: StockReference = StockReference {
    stops: [-0.844_049_04, 0.0, 1.197_313_4],
    black: [0.266_748_8, 0.266_685_3, 0.266_500_2],
    white: [0.538_642_35, 0.595_145_2, 0.620_921_6],
};

/// The reference format 37 stored for `stock`: Portra 800 was the one stock it named, so any other reads as format 38's.
fn former(stock: VirtualStock) -> StockReference {
    match stock {
        VirtualStock::KodakPortra800 => FORMER_PORTRA_800,
        other => datasheet::former(other),
    }
}

/// The reference a format 37 film stored, the hand's moves on it included, read off its raw keys.
fn stored_reference(film: &Value) -> Option<StockReference> {
    let three = |key: &str| -> Option<[f32; 3]> {
        let a = film.get(key)?.as_array()?;
        let v: Vec<f32> = a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
        <[f32; 3]>::try_from(v).ok()
    };
    Some(StockReference { stops: three("stops")?, black: three("black")?, white: three("white")? })
}

/// The settings on this format's pipeline; `stored` is the document's own `settings` object as written.
pub fn caught_up(version: i64, settings: PipelineSettings, stored: Option<&Value>) -> PipelineSettings {
    if version >= DIGITAL_ON_FILM_VERSION || !settings.mode.is_digital() {
        return settings;
    }
    let film = stored.and_then(|s| s.get("virtualFilm")).filter(|f| f.is_object());
    let stock = settings.virtual_film.map_or(VirtualStock::default(), |f| f.stock);
    let mut out = datasheet::on_former(&settings, stock);
    match film.and_then(stored_reference) {
        // The hand's moves off the former reference, laid on the stock's own.
        Some(held) => {
            let (was, now) = (former(stock), datasheet::former(stock));
            let moved = |h: [f32; 3], w: [f32; 3], n: [f32; 3]| std::array::from_fn(|c| n[c] + h[c] - w[c]);
            let placed = StockReference {
                stops: moved(held.stops, was.stops, now.stops),
                black: moved(held.black, was.black, now.black),
                white: moved(held.white, was.white, now.white),
            };
            out.gains.stops = placed.stops;
            out.levels = placed.levels();
        }
        // A photo on no film rendered no print filter, its white standing for them.
        None => {
            out.balance_filters = None;
            out.print.set_filters([0.0; 3]);
        }
    }
    // Lighting's move kept, on the rest a negative's print stands on.
    out.print.exposure += super::departure::FORMER_REST;
    // The negative's own values, which a digital photo never rendered, start at their rest.
    out.density_ceiling = density_axis::rest(out.decoded_from_mosaic, out.mode);
    (out.separation, out.separation_by_hand) = (separation::FAMILY_REST, false);
    (out.chroma_decompression, out.chroma_decompression_by_hand) = (chroma_decompression::BASE, false);
    out
}
