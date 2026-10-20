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
            Err(e) => self.pairing_status(&format!("The phone link cannot start: {e}")),
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
        self.pairing_status("Starting the link…");
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
            (0, 1) => "One phone paired, none connected.".to_string(),
            (0, n) => format!("{n} phones paired, none connected."),
            _ => format!("Connected: {}.", names.join(", ")),
        };
        let p = w.global::<Pairing>();
        p.set_phones(line.into());
        p.set_can_forget(paired > 0);
    }

    /// Forget Phones: every paired phone cut and forgotten; the sheet's code still pairs a new one.
    pub(super) fn forget_phones(&self) {
        let support = self.state.borrow().support.clone();
        let done = match self.state.borrow_mut().phone.as_mut() {
            Some(p) => p.forget_all(),
            None => phone::forget(&support),
        };
        if let Err(e) = done {
            self.pairing_status(&format!("The paired phones cannot be forgotten: {e}"));
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
            return self.pairing_status("No local network: join the phone's Wi-Fi network, then pair again.");
        }
        let uri = code.to_uri();
        match phone::qr(&uri) {
            Ok((rgba, side)) => {
                let p = w.global::<Pairing>();
                p.set_code(image(&rgba, side, side));
                p.set_uri(uri.into());
                let at: Vec<String> = code.addresses.iter().map(ToString::to_string).collect();
                self.pairing_status(&format!("Scan it from the phone app on the same network. One phone, two minutes · {}", at.join(", ")));
            }
            Err(e) => self.pairing_status(&format!("The code cannot be drawn: {e}")),
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
        p.link.frame = place;
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
                self.pairing_status(&format!("The phone link cannot listen: {e}"));
            }
            phone::Event::Host(event, receipt) => self.phone_event(event, receipt),
        }
    }

    /// A press lands on the store as one command and one undo step, then the window follows.
    pub(super) fn phone_event(self: &Rc<Self>, event: HostEvent, receipt: Receipt) {
        use oni_link::Command as Key;
        let slider = matches!(&event, HostEvent::Command { command: Key::StepParam { .. } | Key::ResetParam { .. }, .. });
        let sliders = self.phone_sliders();
        let was = self.state.borrow().store.settings().flat_field.clone();
        let (out, changed) = {
            let s = &mut *self.state.borrow_mut();
            let place = s.place();
            let revision = s.store.revision();
            let (Some(p), Some(store)) = (s.phone.as_mut(), s.store.editor()) else { return receipt.applied() };
            p.link.frame = place;
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
        }
        match out {
            Some(Outward::Paired { phone, name }) => {
                let support = self.state.borrow().support.clone();
                if let Err(e) = phone::trust(&support, phone) {
                    self.say(&format!("{name} is paired for this session only: {e}"));
                }
                self.pairing_status(&format!("Paired with {name}."));
            }
            Some(Outward::Connected { phone, name }) => {
                timing::line(&format!("phone connected: {name}"));
                self.say(&format!("{name} connected"));
                if let Some(p) = self.state.borrow_mut().phone.as_mut() {
                    p.connected(phone, name);
                }
                self.show_phones();
            }
            Some(Outward::Disconnected { phone }) => {
                timing::line("phone disconnected");
                self.say("The phone disconnected");
                if let Some(p) = self.state.borrow_mut().phone.as_mut() {
                    p.disconnected(phone);
                }
                self.show_phones();
            }
            Some(Outward::Walk(by)) => self.walk(by),
            Some(Outward::Face(face)) => self.phone_faced(oni_core::command::face_of(face)),
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

    /// `walk(by:)`: the neighbour in the open frame's group, opened and selected; the ends stop it.
    pub(super) fn walk(self: &Rc<Self>, by: i8) {
        let target = {
            let s = self.state.borrow();
            let Some((at, count)) = s.place() else { return };
            let to = (i32::from(at) - 1 + i32::from(by)).clamp(0, i32::from(count) - 1) as usize;
            let open = s.open_path().unwrap_or_default();
            let group = s.archive.as_ref().and_then(|a| a.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open)));
            group.and_then(|g| g.entries.get(to)).map(|e| e.path.clone()).filter(|p| p != open)
        };
        let Some(path) = target else { return };
        self.state.borrow_mut().designate(&path);
        self.open_photo(&path);
        self.publish_tiles();
    }
}
