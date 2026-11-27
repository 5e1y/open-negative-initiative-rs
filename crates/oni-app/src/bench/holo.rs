//! The bar's holographic keycap taken off the real window before the year on film is shown: at rest,
//! under the hand and pressed, narrow and wide, found by its foil, the bar's one light and coloured thing.

use super::*;
use crate::Wrapped;
use slint::platform::{PointerEventButton, WindowEvent};

/// What the bar is given to lay itself out again before it is taken, and the window to be drawn at all.
const LAID: Duration = Duration::from_millis(500);
const DRAWN: Duration = Duration::from_secs(3);
/// The bar lies in the window's top eighth; the hand rests this far across the plate.
const BAR: usize = 8;
const ACROSS: f32 = 0.72;

/// The window's picture now, as 8-bit RGBA and its width.
fn taken(w: &crate::MainWindow) -> Option<(Vec<u8>, usize)> {
    let shot = w.window().take_snapshot().ok()?;
    Some((shot.as_bytes().to_vec(), shot.width() as usize))
}

fn written(folder: &std::path::Path, name: &str, (rgba, width): &(Vec<u8>, usize)) {
    if let Some(data) = crate::wrapped::share::jpeg(rgba, *width, rgba.len() / (*width * 4)) {
        let _ = std::fs::create_dir_all(folder);
        let _ = std::fs::write(folder.join(name), data);
    }
}

/// Where a hand rests on the foil in the picture's bar, most of the way across it, in the picture's pixels:
/// the foil is what is light and coloured there, as no key and no word of the bar is; none without a plate.
fn foil((rgba, width): &(Vec<u8>, usize)) -> Option<(f32, f32)> {
    let bar = rgba.len() / (width * 4) / BAR;
    let inked = |i: &usize| {
        let ink = &rgba[i * 4..i * 4 + 3];
        let (least, most) = (ink.iter().min().copied().unwrap_or(0), ink.iter().max().copied().unwrap_or(0));
        least > 110 && most - least > 20
    };
    let plate: Vec<(usize, usize)> = (0..bar * width).filter(inked).map(|i| (i % width, i / width)).collect();
    let (left, right) = (plate.iter().map(|p| p.0).min()?, plate.iter().map(|p| p.0).max()?);
    let (top, foot) = (plate.iter().map(|p| p.1).min()?, plate.iter().map(|p| p.1).max()?);
    Some((left as f32 + (right - left) as f32 * ACROSS, (top + foot) as f32 / 2.0))
}

/// The window narrow, where the bar folds, and wide, where every word has its room, in points.
const SIZES: [(&str, (f32, f32)); 2] = [("narrow", (1100.0, 680.0)), ("wide", (1440.0, 900.0))];

/// The key's looks written into `folder`, then `after`: at rest, under the hand and pressed in the
/// gallery's bar at each size, then at rest in the editor's; at once where the bar holds no key.
pub(super) fn plate(ctl: &Rc<Ctl>, folder: std::path::PathBuf, after: impl FnOnce() + 'static) {
    let Some(w) = ctl.window().filter(|w| !w.global::<Wrapped>().get_title().is_empty()) else { return after() };
    let timer = Rc::new(Timer::default());
    let (me, held, after) = (Rc::downgrade(ctl), timer.clone(), RefCell::new(Some(after)));
    let (step, at) = (Cell::new(0usize), Cell::new(None::<slint::LogicalPosition>));
    let asked = Instant::now();
    timer.start(TimerMode::Repeated, LAID, move || {
        let (Some(ctl), Some(w)) = (me.upgrade(), w.as_weak().upgrade()) else { return held.stop() };
        if asked.elapsed() < DRAWN {
            return;
        }
        let scale = w.window().scale_factor();
        let sized = |(across, down): (f32, f32)| w.window().set_size(slint::LogicalSize::new(across, down));
        let shot = |name: String| {
            let picture = taken(&w);
            if let Some(picture) = &picture {
                written(&folder, &name, picture);
            }
            picture
        };
        let now = step.replace(step.get() + 1);
        // Each size takes four ticks in the gallery: laid, at rest, under the hand, pressed.
        let (size, look) = (now / 4, now % 4);
        if let Some((name, points)) = SIZES.get(size) {
            match look {
                0 => sized(*points),
                1 => {
                    let rest = shot(format!("holo-{name}-rest.jpg"));
                    at.set(rest.as_ref().and_then(foil).map(|(x, y)| slint::LogicalPosition::new(x / scale, y / scale)));
                    if let Some(position) = at.get() {
                        w.window().dispatch_event(WindowEvent::PointerMoved { position });
                    }
                }
                2 => {
                    shot(format!("holo-{name}-hover.jpg"));
                    if let Some(position) = at.get() {
                        w.window().dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
                    }
                }
                _ => {
                    shot(format!("holo-{name}-pressed.jpg"));
                    // The hand leaves without a click: the bench shows the year itself.
                    w.window().dispatch_event(WindowEvent::PointerExited);
                }
            }
            return;
        }
        // Then the editor's bar, on the gallery's first photo, wide then narrow.
        match now - 4 * SIZES.len() {
            0 => {
                w.global::<crate::Library>().invoke_tile_clicked(0, false, false);
                ctl.act(crate::Act::ToggleMode);
            }
            1..=3 => {}
            4 => {
                shot("holo-editor-wide.jpg".into());
                sized(SIZES[0].1);
            }
            5 => {}
            _ => {
                shot("holo-editor-narrow.jpg".into());
                ctl.act(crate::Act::ToggleMode);
                held.stop();
                if let Some(after) = after.borrow_mut().take() {
                    after();
                }
            }
        }
    });
}
