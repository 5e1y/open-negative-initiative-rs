//! Framing: the crop, its ratio, rotation, mirror and straightening, written through the store.

use super::*;

impl Ctl {
    // MARK: - Framing

    /// The Crop tab open on a photo in the editor: the canvas shows the frame uncropped.
    pub(super) fn cropping(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == 1 && self.state.borrow().open.is_some()
    }

    /// The open frame's raw size in full-resolution pixels; `Geometry` alone applies the quarter turns.
    pub(super) fn image_size(&self) -> Option<(f64, f64)> {
        let s = self.state.borrow();
        let (_, n) = s.negative.as_ref()?;
        let (w, h) = n.size();
        Some((n.full_width, n.full_width * f64::from(h) / f64::from(w.max(1))))
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
            let current = s.store.settings().geometry.clone();
            let base = match &s.straightening {
                Some((base, written, at)) if at.elapsed() < STRAIGHTEN_IDLE && *written == current => base.clone(),
                _ => current,
            };
            let fitted = base.straightened(angle, image);
            s.straightening = Some((base, fitted.clone(), Instant::now()));
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
        let rendered = s.full_size?;
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
        let image = self.image_size();
        let crop_frame = self.image_frame(true);
        let picture_frame = self.image_frame(cropping);
        let (g, armed, board, window_board) = {
            let s = self.state.borrow();
            let board = s.store.editor_ref().is_some_and(|e| e.board.framing.is_some());
            (s.store.settings().geometry.clone(), s.window_armed, board, s.window_board)
        };
        w.global::<PanelState>().set_crop_first(g.crop == Fraction::UNIT && g.measure_window.is_none());
        let f = w.global::<crate::Framing>();
        let ratios = [oni_settings::frame::AspectRatio::ThreeTwo, oni_settings::frame::AspectRatio::FourThree, oni_settings::frame::AspectRatio::Square, oni_settings::frame::AspectRatio::SixSeven, oni_settings::frame::AspectRatio::FiveFour];
        f.set_ratio(g.ratio.and_then(|r| ratios.iter().position(|x| *x == r)).map_or(0, |i| i as i32 + 1));
        f.set_portrait(g.portrait);
        f.set_portrait_enabled(g.ratio.is_some_and(|r| r != oni_settings::frame::AspectRatio::Square));
        f.set_kept(image.map(|i| geom::pixel_label(g.cropped_pixel_size(i))).unwrap_or_default().into());
        f.set_modified(g != Geometry::reset_keeping(&g));
        f.set_mirrored(g.mirrored);
        f.set_mirrored_vertically(g.mirrored_vertically);
        f.set_can_paste(board);
        f.set_armed(armed);
        f.set_has_window(g.measure_window.is_some());
        f.set_can_paste_window(window_board.is_some());
        let c = w.global::<Canvas>();
        c.set_cropping(cropping && crop_frame.is_some());
        if let Some(frame) = crop_frame {
            let (x, y, wd, ht) = frames::shown(g.crop, frame);
            c.set_crop_x(x as f32);
            c.set_crop_y(y as f32);
            c.set_crop_width(wd as f32);
            c.set_crop_height(ht as f32);
            // `liveSize`: the kept pixels above the frame while a handle is held, inside the view.
            let held = self.state.borrow().frame_gesture.is_some_and(|(window, ..)| !window);
            let view = (f64::from(c.get_width()), f64::from(c.get_height()));
            let label = if held { image.map(|i| geom::pixel_label(g.cropped_pixel_size(i))).unwrap_or_default() } else { String::new() };
            c.set_crop_badge(label.into());
            c.set_crop_badge_x((x + wd / 2.0).clamp(BADGE_MARGIN, (view.0 - BADGE_MARGIN).max(BADGE_MARGIN)) as f32);
            c.set_crop_badge_y((y - BADGE_RISE).clamp(BADGE_MARGIN / 2.0, (view.1 - BADGE_MARGIN / 2.0).max(BADGE_MARGIN / 2.0)) as f32);
        }
        let window_frame = if cropping { crop_frame } else { picture_frame };
        c.set_window_shown(armed && window_frame.is_some());
        if let Some(frame) = window_frame {
            let (x, y, wd, ht) = frames::shown(frames::window_shown(&g, cropping), frame);
            c.set_window_x(x as f32);
            c.set_window_y(y as f32);
            c.set_window_width(wd as f32);
            c.set_window_height(ht as f32);
        }
    }

    /// A ratio or an orientation chosen: the frame cut to it in place and remembered for the next.
    pub(super) fn choose_framing(self: &Rc<Self>, change: impl FnOnce(&mut Geometry)) {
        let Some(image) = self.image_size() else { return };
        let mut g = self.state.borrow().store.settings().geometry.clone();
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

    pub(super) fn framing_act(self: &Rc<Self>, action: &str) {
        let Some(image) = self.image_size() else { return };
        let mut g = self.state.borrow().store.settings().geometry.clone();
        match action {
            // Three quarters counter-clockwise is one clockwise, as the key says: the render turns the other way.
            "rotate" => g.rotate(3, image),
            "mirror" => g.mirror(),
            "mirror-vertically" => g.mirror_vertically(),
            "reset-crop" => g = Geometry::reset_keeping(&g),
            "copy-framing" => {
                if let Some(e) = self.state.borrow_mut().store.editor() {
                    e.copy_framing();
                }
                self.board_out(crate::pasteboard::FRAMING);
                return self.publish_framing();
            }
            "paste-framing" | "paste-framing-to-roll" => return self.paste_framing(action == "paste-framing-to-roll"),
            "window-reset" => g.measure_window = None,
            "window-arm" => {
                let armed = {
                    let mut s = self.state.borrow_mut();
                    s.window_armed = !s.window_armed;
                    s.window_armed
                };
                if armed {
                    g.propose_measure_window(self.state.borrow().store.settings().sheet.is_some());
                }
            }
            "window-copy" => {
                let window = g.measure_window;
                self.state.borrow_mut().window_board = window;
                self.board_out(crate::pasteboard::MEASURE_WINDOW);
                return self.publish_framing();
            }
            "window-paste" => {
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

    /// A tab opened: the toolbar's De-dust key lit on De-dust alone, and on Crop the framing the
    /// hand last chose is handed to this frame if chosen on another.
    pub(super) fn tab_opened(self: &Rc<Self>, tab: i32) {
        if let Some(w) = self.window() {
            w.global::<Bar>().set_dedust(tab == 4);
        }
        if tab == 1 {
            let remembered = {
                let s = self.state.borrow();
                let open = s.open_path().map(str::to_string);
                s.framing_memory.as_ref().filter(|(_, on)| Some(on) != open.as_ref()).map(|(r, _)| *r)
            };
            if let (Some(r), Some(image)) = (remembered, self.image_size()) {
                let mut g = self.state.borrow().store.settings().geometry.clone();
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
