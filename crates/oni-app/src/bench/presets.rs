//! `ONI_BENCH=presets`: a preset saved, the deck raised under the pointer, a card previewed and applied.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum Deck {
    Loading,
    Saved { at: Instant },
    Raised { at: Instant },
    Hovered { at: Instant },
    Applied,
}

pub(super) fn pointer_to(w: &crate::MainWindow, at: slint::LogicalPosition) {
    w.window().dispatch_event(slint::platform::WindowEvent::PointerMoved { position: at });
}

pub(super) fn deck(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Deck::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let saved = Cell::new(0.0f32);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let presets = w.global::<crate::Presets>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let exposure = || ctl.state.borrow().store().get(Field::Exposure);
        let shown = || ctl.state.borrow().shown_settings().map(|s| Field::Exposure.knob().get(s));
        // The card's middle, wherever the deck stands now.
        let card = || slint::LogicalPosition::new(presets.get_first_x() + 36.0, presets.get_first_y() + 52.0);
        match phase.get() {
            Deck::Loading => {
                if ctl.state.borrow().renders() < 1 {
                    if started.elapsed() > LOADING {
                        finish("presets: no frame rendered".into());
                    }
                    return;
                }
                // The look saved two stops over the frame's own, then the frame back at rest.
                let rest = exposure();
                let g = w.global::<StoreGlobal>();
                g.invoke_press(Field::Exposure.index(), false);
                g.invoke_drag(Field::Exposure.index(), 30.0, SPAN, false);
                g.invoke_release(Field::Exposure.index());
                saved.set(exposure());
                ctl.act("save-preset");
                presets.invoke_form_named("Bench".into());
                presets.invoke_save_confirmed();
                ctl.set(Field::Exposure, rest);
                phase.set(Deck::Saved { at: Instant::now() });
            }
            Deck::Saved { at } => {
                if at.elapsed() < Duration::from_millis(300) {
                    return;
                }
                // Onto the strip the deck leaves at rest, under the lift's band the photo keeps.
                let bottom = presets.get_first_y() + w.global::<crate::DS>().get_control_card_lift() + 8.0;
                pointer_to(&w, slint::LogicalPosition::new(presets.get_first_x() + 36.0, bottom));
                phase.set(Deck::Raised { at: Instant::now() });
            }
            Deck::Raised { at } => {
                if at.elapsed() < Duration::from_millis(400) {
                    return;
                }
                pointer_to(&w, card());
                phase.set(Deck::Hovered { at: Instant::now() });
            }
            Deck::Hovered { at } => {
                let previewing = presets.get_hovered() == 0 && shown().is_some_and(|e| (e - saved.get()).abs() < 1e-4);
                if previewing && (exposure() - saved.get()).abs() > 1e-3 {
                    let at = card();
                    let window = w.window();
                    window.dispatch_event(slint::platform::WindowEvent::PointerPressed { position: at, button: slint::platform::PointerEventButton::Left });
                    window.dispatch_event(slint::platform::WindowEvent::PointerReleased { position: at, button: slint::platform::PointerEventButton::Left });
                    phase.set(Deck::Applied);
                } else if at.elapsed() > Duration::from_secs(20) {
                    finish(format!(
                        "presets: no preview under the pointer (hovered {}, shown {:?}, saved {}, store {})",
                        presets.get_hovered(),
                        shown(),
                        saved.get(),
                        exposure()
                    ));
                }
            }
            Deck::Applied => {
                let applied = (exposure() - saved.get()).abs() < 1e-4;
                // Taken back, so the test copy's sidecar reads as it did.
                ctl.act("undo");
                finish(if applied {
                    format!("presets: the deck rose, the card previewed {:.2} EV under the pointer and its click applied it, no failure", saved.get())
                } else {
                    format!("presets: the click applied nothing (store {}, saved {})", exposure(), saved.get())
                });
            }
        }
    });
}
