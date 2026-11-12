//! Framing: the crop, its ratio, rotation, mirror and straightening, written through the store.

use super::*;

impl Ctl {
    // MARK: - Framing

    /// The Crop tab open on a photo alone in the editor: the canvas shows the frame uncropped.
    pub(super) fn cropping(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == crate::PanelTab::Crop && self.state.borrow().frame.is_some() && !self.together_shown()
    }

    /// The open frame's raw size in full-resolution pixels; `Geometry` alone applies the quarter turns.
    pub(super) fn image_size(&self) -> Option<(f64, f64)> {
        Some(self.state.borrow().negative()?.full())
    }

    /// The open frame's kept size in whole pixels, which the crop's readout, its badge and the status
    /// bar all say.
    pub(super) fn kept_size(&self) -> Option<(f64, f64)> {
        let image = self.image_size()?;
        Some(self.state.borrow().store.settings().geometry.kept_pixels(image))
    }

    pub(super) fn write_geometry(self: &Rc<Self>, g: Geometry) {
        let mut next = self.state.borrow().store.settings().clone();
        if next.geometry == g {
            return;
        }
        next.geometry = g;
        self.write(Command::Write(Box::new(next)));
    }

    /// `GeometryPanel.angleBinding`: re-fitted from the gesture's own starting frame, so the
    /// intermediate angles leave no ratchet; a pause or a frame moved elsewhere starts a new one.
    pub(super) fn straighten(self: &Rc<Self>, angle: f32) {
        let Some(image) = self.image_size() else { return };
        let fitted = {
            let mut s = self.state.borrow_mut();
            let current = s.store.settings().geometry;
            let Some(f) = s.frame.as_mut() else { return };
            let base = match &f.straightening {
                Some((base, written, at)) if at.elapsed() < STRAIGHTEN_IDLE && *written == current => *base,
                _ => current,
            };
            let fitted = base.straightened(angle, image);
            f.straightening = Some((base, fitted, Instant::now()));
            fitted
        };
        self.write_geometry(fitted);
    }

    /// The image frame the overlays lie on, in points on the canvas: the upright oriented box while
    /// framing, the rendered picture otherwise, both centred where the viewport puts the render.
    pub(super) fn image_frame(&self, framing: bool) -> Option<(f64, f64, f64, f64)> {
        let w = self.window()?;
        let backing = f64::from(w.window().scale_factor());
        let view = self.canvas_size()?;
        let image = self.image_size()?;
        let s = self.state.borrow();
        let rendered = s.full_size()?;
        let full = if framing { s.store.settings().geometry.oriented_size(image) } else { rendered };
        let scale = s.viewport.effective_scale(rendered, view) / backing;
        let (cx, cy) = Viewport::centre((view.0 / backing, view.1 / backing), (s.viewport.offset.0 / backing, s.viewport.offset.1 / backing));
        let size = (full.0 * scale, full.1 * scale);
        Some((cx - size.0 / 2.0, cy - size.1 / 2.0, size.0, size.1))
    }

    /// The Crop tab and the canvas's frames from the open frame's geometry.
    pub(super) fn publish_framing(&self) {
        let Some(w) = self.window() else { return };
        self.board_in();
        let cropping = self.cropping();
        let crop_frame = self.image_frame(true);
        let picture_frame = self.image_frame(cropping);
        let window_frame = if cropping { crop_frame } else { picture_frame };
        let tab = w.global::<PanelState>().get_tab();
        let armed = self.state.borrow().route.armed;
        let crop_shown = cropping && crop_frame.is_some();
        // The window is the open frame's own: a sheet of frames together shows none.
        let window_shown = crate::tabs::window_shown(armed, tab) && window_frame.is_some() && !self.together_shown();
        // A frame held lives as long as its overlay: Slint drops the grab with it and sends no release.
        let orphaned = self.held_frame().is_some_and(|h| !if h.window { window_shown } else { crop_shown });
        if orphaned && self.end_frame_gesture() {
            self.publish_bar();
        }
        let (g, board, window_board) = {
            let s = self.state.borrow();
            let board = s.store.board.framing.is_some();
            (s.store.settings().geometry, board, s.window_board)
        };
        w.global::<PanelState>().set_crop_first(g.crop == Fraction::UNIT && g.measure_window.is_none());
        let f = w.global::<crate::Framing>();
        let ratios = [oni_settings::frame::AspectRatio::ThreeTwo, oni_settings::frame::AspectRatio::FourThree, oni_settings::frame::AspectRatio::Square, oni_settings::frame::AspectRatio::SixSeven, oni_settings::frame::AspectRatio::FiveFour];
        f.set_ratio(g.ratio.and_then(|r| ratios.iter().position(|x| *x == r)).map_or(0, |i| i as i32 + 1));
        f.set_portrait(g.portrait);
        f.set_orientable(g.ratio.is_some_and(|r| r != oni_settings::frame::AspectRatio::Square));
        let kept = self.kept_size().map(geom::pixel_label).unwrap_or_default();
        f.set_kept(kept.as_str().into());
        let (low, high) = Geometry::ANGLE_RANGE;
        f.set_angle_lowest(low.round() as i32);
        f.set_angle_highest(high.round() as i32);
        f.set_modified(g != Geometry::reset_keeping(&g));
        f.set_mirrored(g.mirrored);
        f.set_mirrored_vertically(g.mirrored_vertically);
        f.set_can_paste(board);
        f.set_armed(armed);
        f.set_has_window(g.measure_window.is_some());
        f.set_can_paste_window(window_board.is_some());
        let c = w.global::<Canvas>();
        c.set_cropping(crop_shown);
        if let Some(frame) = crop_frame {
            let (x, y, wd, ht) = frames::shown(g.crop, frame);
            c.set_crop_x(x as f32);
            c.set_crop_y(y as f32);
            c.set_crop_width(wd as f32);
            c.set_crop_height(ht as f32);
            // `liveSize`: the kept pixels above the frame while a handle is held, inside the view.
            let held = self.held_frame().is_some_and(|h| !h.window);
            let view = (f64::from(c.get_width()), f64::from(c.get_height()));
            c.set_crop_badge(if held { kept.as_str() } else { "" }.into());
            c.set_crop_badge_x((x + wd / 2.0).clamp(BADGE_MARGIN, (view.0 - BADGE_MARGIN).max(BADGE_MARGIN)) as f32);
            c.set_crop_badge_y((y - BADGE_RISE).clamp(BADGE_MARGIN / 2.0, (view.1 - BADGE_MARGIN / 2.0).max(BADGE_MARGIN / 2.0)) as f32);
        }
        c.set_window_shown(window_shown);
        if let Some(frame) = window_frame {
            let (x, y, wd, ht) = frames::shown(frames::window_shown(&g, cropping), frame);
            c.set_window_x(x as f32);
            c.set_window_y(y as f32);
            c.set_window_width(wd as f32);
            c.set_window_height(ht as f32);
        }
    }

    /// A frame on the canvas held, moved and let go: one undo step, constrained on every event.
    pub(super) fn frame_grabbed(self: &Rc<Self>, window: bool, handle: i32) {
        let Some(handle) = Handle::from_index(handle) else { return };
        let cropping = self.cropping();
        let Some(frame) = self.image_frame(cropping || !window) else { return };
        let mut s = self.state.borrow_mut();
        let g = s.store.settings().geometry;
        let from = if window { frames::window_shown(&g, cropping) } else { g.crop };
        let Some(f) = s.frame.as_mut() else { return };
        f.frame_gesture = Some(FrameGesture { window, handle, from, size: (frame.2, frame.3), spent: false });
        s.store.hold();
    }

    pub(super) fn frame_dragged(self: &Rc<Self>, dx: f32, dy: f32) {
        let Some(image) = self.image_size() else { return };
        let cropping = self.cropping();
        let Some(held) = self.held_frame().filter(|h| !h.spent) else { return };
        let proposed = frames::apply(held.handle, held.from, held.size, (f64::from(dx), f64::from(dy)));
        let g = self.state.borrow().store.settings().geometry;
        let next = if held.window {
            Geometry { measure_window: Some(frames::window_stored(&g, frames::bounded(proposed), cropping)), ..g }
        } else {
            frames::constrained_crop(&g, held.handle, proposed, image)
        };
        self.write_geometry(next);
        self.publish_framing();
    }

    /// `CropOverlay`'s Esc: the held frame back where the press found it, the drag spent.
    pub(super) fn cancel_frame_drag(self: &Rc<Self>) -> bool {
        let cropping = self.cropping();
        let (window, from) = {
            let mut s = self.state.borrow_mut();
            let Some(held) = s.frame.as_mut().and_then(|f| f.frame_gesture.as_mut()).filter(|h| !h.spent) else { return false };
            held.spent = true;
            (held.window, held.from)
        };
        let g = self.state.borrow().store.settings().geometry;
        let back = if window { Geometry { measure_window: Some(frames::window_stored(&g, from, cropping)), ..g } } else { Geometry { crop: from, ..g } };
        self.write_geometry(back);
        self.publish_framing();
        true
    }

    pub(super) fn frame_released(self: &Rc<Self>) {
        if self.end_frame_gesture() {
            self.publish(false);
        }
    }

    /// The held frame let go, by the hand, its overlay gone or its photo left: the drag is one step.
    pub(super) fn end_frame_gesture(&self) -> bool {
        let mut s = self.state.borrow_mut();
        let ended = s.frame.as_mut().and_then(|f| f.frame_gesture.take()).is_some();
        if ended {
            s.store.release();
        }
        ended
    }

    /// The frame held on the canvas, a crop's or the measuring window's.
    fn held_frame(&self) -> Option<FrameGesture> {
        self.state.borrow().frame.as_ref()?.frame_gesture
    }

    /// An arrow beside the crop moves its frame rather than walk the photos, as on the canvas.
    pub(super) fn nudge_frame(self: &Rc<Self>, text: &str, command: bool, shift: bool, option: bool, bare: bool) -> bool {
        (bare || ((shift || option) && !command)) && self.cropping() && self.place_frame(text, shift, option)
    }

    /// A key on the canvas holding the keyboard: Prepare's floor places its picker or its window, the
    /// editor's canvas the frame it draws and Return keeps it; a key neither takes goes on to the photos.
    pub(super) fn canvas_key(self: &Rc<Self>, text: &str, shift: bool, option: bool) -> bool {
        if self.state.borrow().prepare.shown {
            return self.floor_key(text, shift, option);
        }
        self.validate_key(text, !shift && !option, false) || self.place_frame(text, shift, option)
    }

    /// An arrow on the frame drawn innermost, the measuring window while it shows, else the crop while
    /// cropping: its body, or with ⌥ its right and bottom edges; the steps that follow each other are
    /// one undo step, as a slider's are.
    fn place_frame(self: &Rc<Self>, text: &str, shift: bool, option: bool) -> bool {
        let Some(w) = self.window() else { return false };
        let Some((handle, by)) = frames::arrow(text, shift, option) else { return false };
        let (cropping, armed) = (self.cropping(), self.state.borrow().route.armed);
        let window = crate::tabs::window_shown(armed, w.global::<PanelState>().get_tab()) && !self.together_shown();
        if !(cropping || window) {
            return false;
        }
        let (Some(image), Some(frame)) = (self.image_size(), self.image_frame(cropping || !window)) else { return true };
        let g = self.state.borrow().store.settings().geometry;
        let next = if window {
            let moved = frames::apply(handle, frames::window_shown(&g, cropping), (frame.2, frame.3), by);
            Geometry { measure_window: Some(frames::window_stored(&g, frames::bounded(moved), cropping)), ..g }
        } else {
            frames::constrained_crop(&g, handle, frames::apply(handle, g.crop, (frame.2, frame.3), by), image)
        };
        self.state.borrow_mut().store.defer();
        self.write_geometry(next);
        self.publish_framing();
        true
    }

    /// A ratio or an orientation chosen: the frame cut to it in place and remembered for the next.
    pub(super) fn choose_framing(self: &Rc<Self>, change: impl FnOnce(&mut Geometry)) {
        let Some(image) = self.image_size() else { return };
        let mut g = self.state.borrow().store.settings().geometry;
        change(&mut g);
        g.apply_ratio(image, None, geom::Driving::Width);
        g.fit_crop_inside_image(image);
        {
            let mut s = self.state.borrow_mut();
            let path = s.open_path().map(str::to_string).unwrap_or_default();
            s.framing_memory = Remembered::new(g.ratio, g.portrait).map(|r| (r, path));
        }
        self.write_geometry(g);
    }

    pub(super) fn framing_act(self: &Rc<Self>, action: crate::Act) {
        if self.together_shown() {
            return;
        }
        let Some(image) = self.image_size() else { return };
        let mut g = self.state.borrow().store.settings().geometry;
        match action {
            // Three quarters counter-clockwise is one clockwise, as the key says: the render turns the other way.
            crate::Act::Rotate => g.rotate(3, image),
            crate::Act::Mirror => g.mirror(),
            crate::Act::MirrorVertically => g.mirror_vertically(),
            crate::Act::ResetCrop => g = Geometry::reset_keeping(&g),
            crate::Act::WindowReset => g.measure_window = None,
            crate::Act::WindowArm => {
                let armed = {
                    let mut s = self.state.borrow_mut();
                    s.route.armed = !s.route.armed;
                    s.route.armed
                };
                if armed {
                    g.propose_measure_window(self.state.borrow().store.settings().sheet.is_some());
                }
            }
            crate::Act::WindowCopy => {
                let window = g.measure_window;
                self.state.borrow_mut().window_board = window;
                self.board_out(crate::pasteboard::MEASURE_WINDOW);
                return self.publish_framing();
            }
            crate::Act::WindowPaste => {
                self.board_in();
                let Some(window) = self.state.borrow().window_board else { return };
                if g.measure_window == Some(window) {
                    return;
                }
                g.measure_window = Some(window);
            }
            _ => return,
        }
        self.write_geometry(g);
        self.publish_framing();
    }

    /// `pasteFraming()` on the selection, else the open photo, or `pasteFramingToRoll()`; one gallery step.
    pub(super) fn paste_framing(self: &Rc<Self>, roll: bool) {
        self.board_in();
        if roll {
            self.paste_framing_to_roll();
        } else {
            let (targets, size) = (self.targets(), self.image_size());
            self.gesture(&targets, |store| store.paste_framing(&targets, size));
        }
        self.publish_framing();
    }

    /// A tab opened: on Crop the framing the hand last chose is handed to this frame if chosen on another.
    pub(super) fn tab_opened(self: &Rc<Self>, tab: crate::PanelTab) {
        let left = self.state.borrow().route.shown;
        self.route_moved(tab);
        self.balance_on_leaving(left, tab);
        if tab == crate::PanelTab::Crop {
            // Crop's handles take the deck's edge, and the deck with its preview.
            self.clear_hover();
            let remembered = {
                let s = self.state.borrow();
                let open = s.open_path().map(str::to_string);
                s.framing_memory.as_ref().filter(|(_, on)| Some(on) != open.as_ref()).map(|(r, _)| *r)
            };
            if let (Some(r), Some(image)) = (remembered, self.image_size()) {
                let mut g = self.state.borrow().store.settings().geometry;
                if (g.ratio, g.portrait) != (Some(r.ratio), r.portrait) {
                    g.adopt(r, image);
                    self.write_geometry(g);
                }
            }
        }
        self.publish_framing();
        self.request_render();
    }
}

/// A frame held on the canvas: the window's or the crop's, the handle, the frame at the press, the
/// shown image frame's size in points, and whether Esc put it back, so the rest of the drag moves nothing.
#[derive(Clone, Copy)]
pub(super) struct FrameGesture {
    window: bool,
    handle: Handle,
    from: Fraction,
    size: (f64, f64),
    spent: bool,
}
