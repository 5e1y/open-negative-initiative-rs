//! The editors' callbacks from the window, each handed to its gesture.

use super::*;

pub fn wire(ctl: &Rc<Ctl>, window: &MainWindow) {
    let g = window.global::<Global>();

    let p = ctl.clone();
    g.on_resized(move |plot, w, h| {
        let Some(plot) = Plot::from_index(plot) else { return };
        let size = Size::new(f64::from(w), f64::from(h));
        {
            let mut gestures = p.editors.gestures.borrow_mut();
            if gestures.sizes[plot as usize] == size {
                return;
            }
            gestures.sizes[plot as usize] = size;
        }
        draw(&p, false);
        p.plot_resized(plot);
    });

    let p = ctl.clone();
    g.on_placed(move |plot, x, y| {
        let Some(plot) = Plot::from_index(plot) else { return };
        p.editors.gestures.borrow_mut().origins[plot as usize] = (f64::from(x), f64::from(y));
    });

    let p = ctl.clone();
    g.on_pointer(move |plot, kind, x, y, alt| {
        let Some(plot) = Plot::from_index(plot) else { return };
        let at = (f64::from(x), f64::from(y));
        let now = Instant::now();
        let then = run(&p, |g, store| match kind {
            0 => g.press(store, plot, at, now),
            1 => g.drag(store, plot, at, alt, now),
            2 | 3 => g.release(store, plot),
            4 if plot == Plot::Wheel => g.hover(store, Some(at)),
            5 if plot == Plot::Wheel => g.hover(store, None),
            _ => Then::Nothing,
        });
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_scrolled(move |plot, x, dx, dy, alt| {
        let Some(plot) = Plot::from_index(plot) else { return false };
        let mut claimed = false;
        let then = run(&p, |g, store| match g.scroll(store, plot, f64::from(x), f64::from(dx), f64::from(dy), alt) {
            Some(then) => {
                claimed = true;
                then
            }
            None => Then::Nothing,
        });
        after(&p, then);
        claimed
    });

    let p = ctl.clone();
    g.on_key(move |plot, text, alt, _shift| {
        if Plot::from_index(plot) != Some(Plot::Wheel) {
            return false;
        }
        let mut claimed = false;
        let then = run(&p, |g, store| match g.key(store, &text, alt) {
            Some(then) => {
                claimed = true;
                then
            }
            None => Then::Nothing,
        });
        after(&p, then);
        claimed
    });

    let p = ctl.clone();
    g.on_focus_changed(move |plot, on| {
        if Plot::from_index(plot) != Some(Plot::Wheel) {
            return;
        }
        {
            let mut gestures = p.editors.gestures.borrow_mut();
            if gestures.focused == on {
                return;
            }
            gestures.focused = on;
        }
        draw(&p, false);
    });

    let p = ctl.clone();
    g.on_pick(move |which, tab| {
        let then = p.editors.gestures.borrow_mut().pick(which, tab.max(0) as usize);
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_reset(move |block| {
        let block = match block {
            0 => Reset::Curves,
            1 => Reset::Levels,
            2 => Reset::Spectrogram,
            3 => Reset::Saturation,
            _ => return,
        };
        let then = run(&p, |g, store| g.reset(store, block));
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_slider_press(move |field, _fine| {
        let mut double = false;
        let then = run(&p, |g, store| {
            let (d, then) = g.band_press(store, field.max(0) as usize, Instant::now());
            double = d;
            then
        });
        after(&p, then);
        double
    });

    let p = ctl.clone();
    g.on_slider_drag(move |field, translation, span, fine| {
        let then = run(&p, |g, store| g.band_drag(store, field.max(0) as usize, translation, span, fine, Instant::now()));
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_slider_release(move |_field| {
        let then = run(&p, |g, store| g.band_release(store));
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_slider_scroll(move |field, dx, dy, span, fine| {
        let mut claimed = false;
        let then = run(&p, |g, store| {
            claimed = g.band_scroll(store, field.max(0) as usize, dx, dy, span, fine);
            if claimed { Then::Publish } else { Then::Nothing }
        });
        after(&p, then);
        claimed
    });

    let p = ctl.clone();
    g.on_slider_step(move |field, direction| {
        let then = run(&p, |g, store| g.band_step(store, field.max(0) as usize, direction));
        after(&p, then);
    });

    // The plots laid out before these callbacks existed report their size and place again.
    g.set_epoch(g.get_epoch() + 1);
}
