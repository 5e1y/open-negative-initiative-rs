//! `ONI_BENCH=grid`: the gallery's frames, up to 36, opened together, then Exposure dragged over them;
//! `ONI_GRID_ZOOM=1` drags zoomed in as far as the canvas goes, where a few tiles show.

use super::*;

/// Frames a grid holds at most, a roll's worth.
const FRAMES: usize = 36;
/// Notches dragged: four seconds at a trackpad's pace, an eighth of the usual step each, inside the range.
const DRAG: usize = 8 * NOTCHES;

/// Every frame picked by the hand's clicks and opened together; once each tile is decoded, `DRAG` notches
/// at a trackpad's pace, then each grid's latency from its notch to the canvas, and the drag undone.
pub(super) fn grid(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let drawn = frame_clock(ctl);
    let dragged = Cell::new(None::<Instant>);
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let phase = Cell::new(0usize);
    let opened = Cell::new(None::<Instant>);
    let decoded = Cell::new(None::<Duration>);
    let from = Cell::new(0usize);
    let notched = Cell::new(0usize);
    let zoom = Cell::new(std::env::var_os("ONI_GRID_ZOOM").is_some());
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        let g = w.global::<StoreGlobal>();
        let field = Field::Exposure.index();
        let n = phase.get();
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 {
                    return;
                }
                ctl.act("toggle-mode");
                let lib = w.global::<crate::Library>();
                let mut tiles = crate::app::tiles(&w);
                tiles.sort_by_key(|t| t.index);
                let picked: Vec<_> = tiles.iter().filter(|t| !t.caption.contains("contact sheet")).take(FRAMES).collect();
                for (i, t) in picked.iter().enumerate() {
                    lib.invoke_tile_clicked(t.index, i > 0, false);
                }
                lib.invoke_tile_opened(picked[0].index);
                opened.set(Some(Instant::now()));
                phase.set(1);
            }
            1 => {
                let (tiles, sources, latencies) = ctl.together_stats();
                if tiles > 0 && sources == tiles && zoom.replace(false) {
                    let c = w.global::<crate::Canvas>();
                    c.invoke_pinch_started();
                    c.invoke_pinched(100.0, c.get_width() / 2.0, c.get_height() / 2.0);
                    return;
                }
                if tiles > 0 && sources == tiles {
                    decoded.set(opened.get().map(|t| t.elapsed()));
                    from.set(latencies.len());
                    notched.set(ctl.state.borrow().notches().count());
                    dragged.set(Some(Instant::now()));
                    g.invoke_press(field, false);
                    phase.set(2);
                }
            }
            k if k < 2 + DRAG => {
                g.invoke_drag(field, (k - 1) as f32 * STEP / 8.0, SPAN, false);
                phase.set(k + 1);
            }
            k if k == 2 + DRAG => {
                g.invoke_release(field);
                phase.set(k + 1);
            }
            _ => {
                let (tiles, _, latencies) = ctl.together_stats();
                let latencies = &latencies[from.get().min(latencies.len())..];
                if latencies.is_empty() || ctl.state.borrow().store().revision() != ctl.together_drawn() {
                    return;
                }
                let mut ms: Vec<f64> = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
                ms.sort_by(f64::total_cmp);
                let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
                let text = format!(
                    "bench, grid: {tiles} frames together, sources in {:.1} s; {} grids over {DRAG} notches, notch to canvas p50 {:.1} ms, p99 {:.1} ms, max {:.1} ms",
                    decoded.get().unwrap_or_default().as_secs_f64(),
                    ms.len(),
                    at(0.5),
                    at(0.99),
                    ms[ms.len() - 1],
                );
                // The interface's own pace over the drag: each frame drawn, its gaps and spikes.
                let text = format!("{text}\n{}", ctl.state.borrow().notches().cadence(notched.get()));
                let text = format!("{text}\n{}", frames_since(&drawn.borrow(), dragged.get().unwrap_or_else(Instant::now)));
                ctl.act("undo");
                ctl.flush();
                held.stop();
                if let Some(done) = done.borrow_mut().take() {
                    done(text);
                }
            }
        }
    });
}
