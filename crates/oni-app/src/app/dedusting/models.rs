//! De-dust's models on the window: each kept across a publish and written row by row where it changed.

use slint::{Image, Model, ModelRc, VecModel};

/// `items` written into `model` row by row where they differ, a model of its own given when it has none:
/// a repeater keeps every element whose row holds, where a new model builds them all again.
pub(super) fn held<T: Clone + 'static>(model: &ModelRc<T>, items: Vec<T>, same: impl Fn(&T, &T) -> bool) -> Option<ModelRc<T>> {
    let Some(rows) = model.as_any().downcast_ref::<VecModel<T>>() else { return Some(ModelRc::new(VecModel::from(items))) };
    for row in (items.len()..rows.row_count()).rev() {
        rows.remove(row);
    }
    for (row, item) in items.into_iter().enumerate() {
        match rows.row_data(row) {
            Some(shown) if same(&shown, &item) => {}
            Some(_) => rows.set_row_data(row, item),
            None => rows.push(item),
        }
    }
    None
}

/// Two rows showing one tile: Slint holds no two empty images equal, and a reminder not made yet is one.
pub(super) fn same_row(a: &crate::CorrectionRow, b: &crate::CorrectionRow) -> bool {
    let blank = |i: &Image| i.size().width == 0;
    (&a.label, a.number, a.pending, a.fallback) == (&b.label, b.number, b.pending, b.fallback) && (a.original == b.original || (blank(&a.original) && blank(&b.original)))
}

#[cfg(test)]
mod tests {
    use super::{held, same_row};
    use slint::{Model, ModelRc};

    #[test]
    fn a_model_keeps_the_rows_that_hold_and_writes_the_others() {
        use std::rc::Rc;
        let list = |values: &[i32]| -> Vec<Rc<i32>> { values.iter().map(|v| Rc::new(*v)).collect() };
        let same = |a: &Rc<i32>, b: &Rc<i32>| a == b;
        let model = held(&ModelRc::default(), list(&[1, 2, 3]), same).expect("a property's own model is replaced once");
        let rows = || model.iter().collect::<Vec<Rc<i32>>>();
        let before = rows();
        let next = list(&[1, 5]);
        assert!(held(&model, next.clone(), same).is_none(), "a changed list is written into the model it has");
        assert_eq!(rows(), next, "the second row written, the third gone");
        assert!(Rc::ptr_eq(&rows()[0], &before[0]) && Rc::ptr_eq(&rows()[1], &next[1]), "the row that holds is left as it was");
        assert!(held(&model, list(&[1, 5, 8]), same).is_none());
        assert_eq!(rows().len(), 3, "a longer list grows it");
        // Twin: rows never held the same, as two empty images are to Slint, are every one written again.
        let again = list(&[1, 5, 8]);
        assert!(held(&model, again.clone(), |_, _| false).is_none());
        assert!(rows().iter().zip(&again).all(|(a, b)| Rc::ptr_eq(a, b)));
    }

    #[test]
    fn two_rows_whose_reminder_is_not_made_yet_show_one_tile() {
        let row = |number: i32| crate::CorrectionRow { number, ..Default::default() };
        assert!(same_row(&row(1), &row(1)));
        // Twin: Slint's own equality tells them apart, which rebuilt every tile at each notch.
        assert!(row(1) != row(1));
        assert!(!same_row(&row(1), &row(2)), "twin: another number is another tile");
        let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(2, 2);
        let made = crate::CorrectionRow { original: slint::Image::from_rgba8(pixels), ..row(1) };
        assert!(same_row(&made, &made.clone()) && !same_row(&made, &row(1)), "a reminder landed is another row");
    }
}
