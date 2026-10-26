//! The phone link: the host's events, pairing, and the commands the phone sends.

use super::*;
use oni_link::Receipt;

mod panel;

impl Ctl {
    // MARK: - The phone

    /// The link's Host on its own thread, once; its events come back to this thread one by one.
    pub(super) fn start_phone(self: &Rc<Self>) {
        if self.state.borrow().phone.is_some() {
            return;
        }
        let support = self.state.borrow().support.clone();
        match Phone::start(&support, |event| with_shell(move |c| c.phone_said(event))) {
            Ok(p) => self.state.borrow_mut().phone = Some(p),
            Err(e) => self.pairing_status(&tr!("The phone link cannot start: {error}", error = e)),
        }
    }

    /// The sheet with a fresh one-time code, once the Host listens.
    pub(super) fn pair_phone(self: &Rc<Self>) {
        if let Some(w) = self.window() {
            let p = w.global::<Pairing>();
            p.set_code(Image::default());
            p.set_uri(SharedString::new());
            p.set_shown(true);
        }
        self.pairing_status(tr("Starting the link…"));
        self.start_phone();
        self.show_code();
        self.show_phones();
    }

    /// The sheet's line on the phones: those connected now, else how many are paired.
    pub(super) fn show_phones(&self) {
        let Some(w) = self.window() else { return };
        let (names, paired) = {
            let s = self.state.borrow();
            (s.phone.as_ref().map(Phone::names).unwrap_or_default(), phone::trusted(&s.support).len())
        };
        let line = match (names.len(), paired) {
            (0, 0) => String::new(),
            (0, n) => trn("One phone paired, none connected.", "{n} phones paired, none connected.", n as u64),
            _ => tr!("Connected: {phones}.", phones = names.join(", ")),
        };
        let p = w.global::<Pairing>();
        p.set_phones(line.into());
        p.set_can_forget(paired > 0);
        self.publish_settings();
    }

    /// Forget Phones: every paired phone cut and forgotten; the sheet's code still pairs a new one.
    pub(super) fn forget_phones(&self) {
        let support = self.state.borrow().support.clone();
        let done = match self.state.borrow_mut().phone.as_mut() {
            Some(p) => p.forget_all(),
            None => phone::forget(&support),
        };
        if let Err(e) = done {
            self.pairing_status(&tr!("The paired phones cannot be forgotten: {error}", error = e));
        }
        self.show_phones();
    }

    pub(super) fn show_code(&self) {
        let Some(w) = self.window() else { return };
        if !w.global::<Pairing>().get_shown() {
            return;
        }
        let code = self.state.borrow().phone.as_ref().and_then(Phone::open_pairing);
        let Some(code) = code else { return };
        if code.addresses.is_empty() {
            return self.pairing_status(tr("No local network: join the phone's Wi-Fi network, then pair again."));
        }
        let uri = code.to_uri();
        match phone::qr(&uri) {
            Ok((rgba, side)) => {
                let p = w.global::<Pairing>();
                p.set_code(image(&rgba, side, side));
                p.set_uri(uri.into());
                let at: Vec<String> = code.addresses.iter().map(ToString::to_string).collect();
                self.pairing_status(&tr!("Scan it from the phone app on the same network. One phone, two minutes · {addresses}", addresses = at.join(", ")));
            }
            Err(e) => self.pairing_status(&tr!("The code cannot be drawn: {error}", error = e)),
        }
    }

    pub(super) fn close_pairing(&self) {
        if let Some(p) = self.state.borrow().phone.as_ref() {
            p.close_pairing();
        }
        if let Some(w) = self.window() {
            w.global::<Pairing>().set_shown(false);
        }
    }

    pub(super) fn pairing_status(&self, text: &str) {
        if let Some(w) = self.window() {
            w.global::<Pairing>().set_status(text.into());
        }
    }

    /// The store's screen to the phone, the open frame's place with it; unchanged, nothing is sent.
    pub(super) fn refresh_phone(&self) {
        let sliders = self.phone_sliders();
        let s = &mut *self.state.borrow_mut();
        let place = s.place();
        let (Some(p), Some(store)) = (s.phone.as_mut(), s.store.editor()) else { return };
        p.link.frame = place.map(wire_place);
        p.link.sliders = Box::new(sliders);
        p.link.refresh(store);
    }

    fn phone_sliders(&self) -> panel::PhoneSliders {
        panel::PhoneSliders { on_paper: self.export.borrow().settings.space().paper().is_some() }
    }

    pub(super) fn phone_said(self: &Rc<Self>, event: phone::Event) {
        match event {
            phone::Event::Listening(at) => {
                timing::line(&format!("phone link listening on {at}"));
                if let Some(p) = self.state.borrow_mut().phone.as_mut() {
                    p.listening(at);
                }
                self.refresh_phone();
                self.show_code();
            }
            phone::Event::Failed(e) => {
                timing::line(&format!("phone link failed: {e}"));
                self.state.borrow_mut().phone = None;
                self.pairing_status(&tr!("The phone link cannot listen: {error}", error = e));
            }
            phone::Event::Host(event, receipt) => self.phone_event(event, receipt),
        }
    }

    /// A press lands on the store as one command and one undo step, then the window follows.
    pub(super) fn phone_event(self: &Rc<Self>, event: HostEvent, receipt: Receipt) {
        use oni_link::Command as Key;
        let slider = matches!(&event, HostEvent::Command { command: Key::StepParam { .. } | Key::ResetParam { .. } | Key::SetParam { .. }, .. });
        let sliders = self.phone_sliders();
        let was = self.state.borrow().store.settings().flat_field.clone();
        let (out, changed) = {
            let s = &mut *self.state.borrow_mut();
            let place = s.place();
            let revision = s.store.revision();
            let (Some(p), Some(store)) = (s.phone.as_mut(), s.store.editor()) else { return receipt.applied() };
            p.link.frame = place.map(wire_place);
            p.link.sliders = Box::new(sliders);
            let out = p.link.handle(store, event, receipt);
            (out, store.revision() != revision)
        };
        // A hand's flat-field sliders are kept for the next mask, as the panel's own writes keep them.
        if slider {
            self.remember_flat_field(&was);
        }
        if changed {
            self.state.borrow_mut().last_change = Some(Instant::now());
            self.publish(false);
        } else {
            // A drag's release records its step without a change: the bar's undo follows it.
            self.publish_bar();
        }
        match out {
            Some(Outward::Paired { phone, name }) => {
                let support = self.state.borrow().support.clone();
                // Forget Phones between the pairing and this event already cut it: it stays forgotten.
                if !self.state.borrow().phone.as_ref().is_some_and(|p| p.trusts(&phone)) {
                    return;
                }
                if let Err(e) = phone::trust(&support, phone) {
                    self.say(&tr!("{name} is paired for this session only: {error}", name = name, error = e));
                }
                self.pairing_status(&tr!("Paired with {name}.", name = name));
            }
            Some(Outward::Connected { phone, name, connection }) => {
                timing::line("phone connected");
                let new = self.state.borrow_mut().phone.as_mut().is_some_and(|p| p.connected(phone, connection, name.clone()));
                if new {
                    self.say(&tr!("{name} connected", name = name));
                }
                self.show_phones();
            }
            // A phone back on a new link before its old one ended is still connected.
            Some(Outward::Disconnected { phone, connection }) => {
                timing::line("phone disconnected");
                let gone = self.state.borrow_mut().phone.as_mut().is_some_and(|p| p.disconnected(phone, connection));
                if gone {
                    self.say(tr("The phone disconnected"));
                }
                self.show_phones();
            }
            Some(Outward::Walk(by)) => self.walk(by),
            Some(Outward::Face(face)) => self.phone_faced(crate::link::face_of(face)),
            None => {}
        }
    }

    /// The phone switched the face on the store: kept for the next launch, as the bar's switch keeps it.
    fn phone_faced(self: &Rc<Self>, face: oni_settings::Face) {
        super::lab::remember(&self.state.borrow().support, face);
        if face == oni_settings::Face::Scanner {
            self.clear_hover();
        }
        self.publish_bar();
    }

    /// `walk(by:)`: the neighbour in `labFrames`, opened and selected; the ends stop it.
    pub(super) fn walk(self: &Rc<Self>, by: i8) {
        let target = {
            let s = self.state.borrow();
            let Some((at, count)) = s.place() else { return };
            let to = walked(at, count, by);
            let open = s.open_path().unwrap_or_default();
            s.lab_frames().get(to).map(|e| e.path.clone()).filter(|p| p != open)
        };
        let Some(path) = target else { return };
        self.state.borrow_mut().designate(&path);
        self.open_photo(&path);
        self.publish_tiles();
    }
}

/// The frame's place as the wire carries it, past 65 535 held at the end.
fn wire_place((at, count): (usize, usize)) -> (u16, u16) {
    let saturated = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    (saturated(at), saturated(count))
}

/// The zero-based index `by` frames from the one-based `at` in a group of `count`, the ends stopping it.
fn walked(at: usize, count: usize, by: i8) -> usize {
    let from = at.saturating_sub(1);
    let to = if by < 0 { from.saturating_sub(usize::from(by.unsigned_abs())) } else { from.saturating_add(by as usize) };
    to.min(count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_in_a_group_past_the_wire_s_u16_stays_inside_it() {
        assert_eq!(walked(65_536, 65_536, 1), 65_535, "the last frame of 65 536 stops the walk");
        assert_eq!(walked(65_536, 65_536, -3), 65_532);
        assert_eq!(walked(1, 1, -1), 0);
        assert_eq!(walked(1, 0, 1), 0, "an empty group never panics");
        assert_eq!(wire_place((70_000, 70_000)), (u16::MAX, u16::MAX));
        // Twin: a u16 cast reads a group of 65 536 as empty, and wraps past it.
        assert_eq!((65_536usize as u16, 70_000usize as u16), (0, 4464));
        assert_eq!(wire_place((7, 36)), (7, 36));
    }
}
