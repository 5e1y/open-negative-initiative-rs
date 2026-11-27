//! The window's events and the watchdog the launch sets going once the starter has drawn.

use super::*;

/// The window's events and the watchdog, from the starter on; what needs the shell waits for it.
pub(super) fn watch(window: &MainWindow, shell: &Standing, engine: &Shared) {
    // A thread of its own asks the interface thread to answer, which says what it shows.
    let watched = shell.clone();
    oni_app::watchdog::witness(move || {
        let witness = watched.try_borrow().ok().and_then(|s| s.as_ref().and_then(|s| s.ctl.state.try_borrow().ok().map(|s| s.witness())));
        witness.unwrap_or_else(|| "the starter".into())
    });
    // Asked from the event loop's first turn on: the launch before it runs no loop to answer in.
    slint::Timer::single_shot(std::time::Duration::ZERO, oni_app::watchdog::start);
    let room = window.as_weak();
    window.global::<oni_app::Bar>().on_room_pressed(move || {
        if let Some(w) = room.upgrade() {
            oni_app::chrome::room_pressed(w.window());
        }
    });
    let ctl = {
        let shell = shell.clone();
        move || shell.try_borrow().ok().and_then(|s| s.as_ref().map(|s| s.ctl.clone()))
    };
    let cursor = std::cell::Cell::new((0.0, 0.0));
    let (lit, told, gate) = (window.as_weak(), window.as_weak(), engine.clone());
    // winit names each file of one drop in turn: the drop is taken whole once every name is in.
    let drops = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    // Slint reconfigures the surface while it handles these two events: no other thread submits meanwhile.
    window.window().on_winit_window_event(move |window, event| {
        if let WindowEvent::CursorMoved { position, .. } = event {
            cursor.set((position.x, position.y));
        }
        if let WindowEvent::KeyboardInput { event: key, .. } = event
            && key.state == ElementState::Pressed
            && let Some(w) = told.upgrade()
        {
            oni_app::outside::keyed(w.window(), || w.invoke_refocus());
        }
        if let WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } = event {
            if let Some(w) = told.upgrade() {
                oni_app::outside::pressed(&w);
            }
            if *button == MouseButton::Left
                && let Some(pressed) = ctl()
            {
                let (scale, (x, y)) = (f64::from(window.scale_factor()), cursor.get());
                oni_app::editors::pressed_anywhere(&pressed, (x / scale, y / scale));
            }
        }
        // Before Slint's dispatch, which tells a slider destroyed under the hand nothing of the release.
        if let WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } = event
            && let Some(lifted) = ctl()
        {
            lifted.lifted();
        }
        if let WindowEvent::Focused(focused) = event
            && let Some(focus) = ctl()
        {
            focus.focus_changed(*focused);
        }
        if let WindowEvent::DroppedFile(path) = event
            && let Some(dropping) = ctl()
        {
            let first = drops.borrow().is_empty();
            drops.borrow_mut().push(path.clone());
            if first {
                let drops = drops.clone();
                slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                    let paths = std::mem::take(&mut *drops.borrow_mut());
                    dropping.dropped(&paths);
                });
            }
        }
        if let WindowEvent::MouseWheel { delta, .. } = event {
            let wheel = match *delta {
                MouseScrollDelta::LineDelta(dx, dy) => viewport::lines(dx, dy, cfg!(windows)),
                MouseScrollDelta::PixelDelta(p) => {
                    let scale = f64::from(window.scale_factor());
                    Wheel { dx: p.x / scale, dy: p.y / scale, precise: true }
                }
            };
            viewport::note_wheel(wheel);
        }
        if let WindowEvent::Occluded(occluded) = event {
            timing::line(if *occluded { "window occluded" } else { "window visible" });
        }
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
            if let Some(w) = lit.upgrade() {
                lights(&w);
            }
            oni_app::on_screen::settling(lit.clone());
            gate.hold();
            let open = gate.clone();
            let _ = slint::invoke_from_event_loop(move || open.release());
        }
        EventResult::Propagate
    });
}
