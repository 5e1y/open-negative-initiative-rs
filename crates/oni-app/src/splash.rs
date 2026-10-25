//! `Splash`: a borderless picture centred on the screen at launch, held `DS.Motion.splashMinimum`
//! while the first folder loads underneath, then the main window shown in its place.

use crate::SplashWindow;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer};
use std::time::Duration;

/// The pictures shipped, oldest first, each named after the version it opens.
const PICTURES: [(&str, &str); 2] = [("1.0.0", include_str!("../../oni-ui/ui/splash/1.0.0.txt")), ("1.3.0", include_str!("../../oni-ui/ui/splash/1.3.0.txt"))];
/// `DS.Motion.splashMinimum`.
const MINIMUM: Duration = Duration::from_millis(3330);

/// The version's own picture, else the newest.
pub fn picture(version: &str) -> usize {
    PICTURES.iter().position(|(v, _)| *v == version).unwrap_or(PICTURES.len() - 1)
}

/// The credit printed along the version's picture.
pub fn credit(version: &str) -> String {
    PICTURES[picture(version)].1.trim().to_string()
}

/// The splash on screen, and `reveal` once it has been held long enough.
pub fn show(reveal: impl FnOnce() + 'static) -> Result<(), slint::PlatformError> {
    let version = env!("CARGO_PKG_VERSION");
    let window = SplashWindow::new()?;
    let chosen = picture(version);
    window.set_picture(chosen as i32);
    window.set_line(oni_i18n::tr!("V{version} Loading", version = version).into());
    window.set_credit(PICTURES[chosen].1.trim().into());
    window.show()?;
    window.window().with_winit_window(|w| {
        if let Some(screen) = w.current_monitor() {
            let (s, o, size) = (screen.size(), screen.position(), w.outer_size());
            let x = o.x + (s.width as i32 - size.width as i32) / 2;
            let y = o.y + (s.height as i32 - size.height as i32) / 2;
            w.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(x, y));
        }
    });
    // Held strongly by the timer: an interpreted interface lets a shown window's handle go with `window`.
    let held = window.clone_strong();
    let mut reveal = Some(reveal);
    Timer::single_shot(MINIMUM, move || {
        // The main window first, so the event loop never finds itself without one.
        if let Some(r) = reveal.take() {
            r();
        }
        let _ = held.hide();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_shows_its_own_picture_and_any_other_the_newest() {
        assert_eq!(picture("1.0.0"), 0);
        assert_eq!(picture("1.3.0"), 1);
        // Twin: a version with no picture of its own falls on the newest, never the first.
        assert_eq!(picture("0.1.0"), PICTURES.len() - 1);
        assert!(PICTURES.iter().all(|(_, credit)| !credit.trim().is_empty()));
    }
}
