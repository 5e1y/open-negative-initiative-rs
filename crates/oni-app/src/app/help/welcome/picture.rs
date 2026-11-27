//! The welcome's picture: the screens where the clock has them, the gift drawn by the GPU into the texture the sheet
//! shows, at most one picture in flight, and the screen's film from the moment its words come.

use super::screens::{gift_of, slot};
use crate::app::Ctl;
use crate::stage::direction;
use crate::stage::{Stage, unwrap};
use crate::{Help, StageSpot};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Image, SharedPixelBuffer};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, TryRecvError};

/// The most pixels a picture holds: past it the stage is drawn smaller and the window stretches it.
pub(super) const BUDGET: f32 = 2.6e6;
/// The share of that budget a slow GPU is let down to, and the share of a screen's frame a picture may take.
const LEAST: f32 = 0.3;
const SHARE: f32 = 0.7;
/// How much finer than the window's own pixels the stage may be drawn, each way.
const FINER: f32 = 1.4;

/// The screen's frame, in seconds: a 60 Hz one where the window's screen tells no rate.
fn screen_frame(w: &crate::MainWindow) -> f32 {
    let rate = w.window().with_winit_window(|window| window.current_monitor().and_then(|m| m.refresh_rate_millihertz())).flatten().unwrap_or(60_000);
    1000.0 / (rate as f32).clamp(30_000.0, 240_000.0)
}

impl Ctl {
    /// One picture: the screens where the clock has them, the gift drawn and told to the sheet.
    pub(in crate::app) fn welcome_frame(&self) {
        let Some(w) = self.window() else { return };
        let help = w.global::<Help>();
        let (still, scale) = (w.global::<crate::Motion>().get_reduced(), w.window().scale_factor());
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let Some(play) = state.help.welcome.as_mut() else { return };
        if play.size.0 < 1.0 || play.size.1 < 1.0 {
            return;
        }
        match play.building.as_ref().map(Receiver::try_recv) {
            Some(Ok((built, pace))) => {
                play.most = (SHARE * screen_frame(&w) / pace.max(1e-6) * 1e6).clamp(LEAST * BUDGET, BUDGET);
                (play.stage, play.building) = (Some(built), None);
                crate::timing::line(&format!("welcome: the stage draws a million pixels in {:.1} ms, {:.1} of them a picture", pace * 1000.0, play.most / 1e6));
            }
            Some(Err(TryRecvError::Disconnected)) => {
                play.building = None;
                crate::timing::line("welcome: the stage cannot be drawn on this GPU; its gifts are shown as stills");
            }
            _ => {}
        }
        let now = play.now();
        let cue = play.cue(now);
        let frame = direction::frame(&cue);
        // One picture in flight: the next waits until the GPU has finished the last, so none queues behind it.
        let _ = state.gpu.device.poll(wgpu::PollType::Poll);
        let free = !play.flying.load(Ordering::Acquire);
        // A screen of few pixels is drawn finer than it shows, as far as the budget goes: its edges come out smooth.
        let fit = (play.most / (play.size.0 * play.size.1 * scale * scale)).sqrt().min(FINER) * scale;
        let size = ((play.size.0 * fit).round().max(8.0) as u32, (play.size.1 * fit).round().max(8.0) as u32);
        if let Some(stage) = play.stage.as_mut().filter(|_| free) {
            let gpu = &state.gpu;
            let fresh = play.target.as_ref().is_none_or(|t| (t.width(), t.height()) != size);
            if fresh {
                play.target = Some(Stage::target(&gpu.device, size.0, size.1));
            }
            let Some(target) = &play.target else { return };
            stage.draw(&gpu.device, &gpu.queue, &frame, target);
            play.flying.store(true, Ordering::Release);
            let flying = play.flying.clone();
            gpu.queue.on_submitted_work_done(move || flying.store(false, Ordering::Release));
            if gpu.reads_back {
                if let Some(bytes) = crate::display::read_back(&gpu.device, &gpu.queue, target, size.0, size.1) {
                    help.set_stage(Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&bytes, size.0, size.1)));
                }
            } else if fresh {
                match Image::try_from(target.clone()) {
                    Ok(image) => help.set_stage(image),
                    Err(e) => crate::timing::line(&format!("welcome: the stage's texture refused: {e}")),
                }
            }
            play.revision = play.revision.wrapping_add(1);
            help.set_stage_revision(play.revision);
        }
        help.set_stage_live(play.stage.is_some());
        help.set_welcome_page(play.page);
        help.set_welcome_unwrapped(i32::from(play.bits()));
        // The screens glide as one list, the stage's picture with the screen whose gift it draws.
        let scroll = play.scroll(now, still);
        help.set_scroll(scroll);
        help.set_stage_offset(slot(play.staged(now)) - scroll);
        help.set_welcome_from(play.from);
        help.set_from_opened(gift_of(play.from).is_none_or(|i| play.opened[i].is_some()));
        help.set_stage_shown(cue.shown * cue.gift.map_or(0.0, |i| direction::kept(i, cue.since)));
        help.set_message_turn(play.turned(now, still));
        help.set_gift_shown(cue.gift.map_or(-1, |i| i as i32));
        let since = gift_of(play.page).and_then(|i| play.opened[i]).map_or(-1.0, |o| (now - o).max(0.0));
        help.set_opened_time(since);
        help.set_words_at(gift_of(play.page).map_or(0.0, |i| if still { 0.0 } else { unwrap::title(i) }));
        help.set_may_go_on(play.may_go_on(now, still));
        // The screen's film, from the moment its words come.
        let reel = gift_of(play.page).and_then(|i| play.reels.iter_mut().find(|r| r.gift == i).map(|r| (r, i)));
        let showing = help.get_reel_gift();
        // The film left keeps its last frame on the screen gliding away.
        if showing >= 0 && reel.as_ref().is_none_or(|(_, i)| *i as i32 != showing) {
            help.set_reel_from(help.get_reel());
            help.set_reel_from_gift(showing);
        }
        help.set_reel_gift(reel.as_ref().map_or(-1, |(_, i)| *i as i32));
        if let Some((reel, i)) = reel {
            // Another film's frame is in the picture: this one's is given again.
            if showing != i as i32 {
                reel.again();
            }
            let words = if still { 0.0 } else { unwrap::title(i) };
            if let Some(frame) = reel.at(since - words - 0.4, still) {
                help.set_reel(frame);
            }
        }
        let spot = direction::gift_spot(&frame, cue.aspect).unwrap_or_default();
        help.set_gift_spot(StageSpot { x: spot[0] * play.size.0, y: spot[1] * play.size.1, width: spot[2] * play.size.0, height: spot[3] * play.size.1 });
    }
}
