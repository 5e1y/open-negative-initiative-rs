//! The info sidebar: the roll's or the project's name, stock, date, body and description.

use super::*;

impl Ctl {
    pub(in crate::app) fn publish_info(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let mut s = self.state.borrow_mut();
        if s.org.projects_surface {
            let Some(group) = s.project_group() else {
                lib.set_info_kind(0);
                lib.set_info_empty(tr("Select a project").into());
                s.org.info_loaded = None;
                return;
            };
            lib.set_info_kind(2);
            lib.set_info_title(group.project.name.clone().into());
            let stocks: Vec<StockCount> = oni_library::info::stocks(&group.entries)
                .into_iter()
                .map(|(k, n)| {
                    let artwork = crate::stocks::icon(k.icon_name);
                    StockCount {
                        name: k.display_name.into(),
                        count: n.to_string().into(),
                        has_artwork: artwork.is_some(),
                        artwork: artwork.unwrap_or_default(),
                    }
                })
                .collect();
            lib.set_stocks(ModelRc::new(VecModel::from(stocks)));
            let stack: Vec<Image> = group
                .entries
                .iter()
                .take(3)
                .map(|e| s.thumbs.get(&s.tile_key(e)).unwrap_or_default())
                .collect();
            lib.set_stack(ModelRc::new(VecModel::from(stack)));
            let key = format!("project {}", group.id);
            if s.org.info_loaded.as_deref() != Some(key.as_str()) {
                lib.set_description(group.project.description.clone().into());
                s.org.info_loaded = Some(key);
            }
            return;
        }
        let Some(roll) = s.roll().filter(|_| !s.trash_shown()) else {
            lib.set_info_kind(0);
            lib.set_info_empty(tr("Select a roll").into());
            s.org.info_loaded = None;
            return;
        };
        let film = &roll.film;
        lib.set_info_kind(1);
        lib.set_info_title(film.name.clone().into());
        // `heroStock`: the stock a hand picked, else the first photo's mode's default, said so.
        let mode = roll.first.as_ref().and_then(|e| e.mode);
        let explicit = film_stock(film.stock.as_deref());
        let fallback = mode.and_then(|m| film_stock(Some(default_stock_id(m))));
        let (stock, note) = match (explicit, fallback, mode) {
            (Some(k), _, _) => (k.display_name.to_string(), String::new()),
            (None, Some(k), Some(m)) => (k.display_name.to_string(), tr!("Default, from this roll's {mode} mode", mode = m.label())),
            _ => (String::new(), tr("No stock set").to_string()),
        };
        let artwork = explicit.or(fallback).and_then(|k| crate::stocks::icon(k.icon_name));
        lib.set_stock_has_artwork(artwork.is_some());
        lib.set_stock_artwork(artwork.unwrap_or_default());
        lib.set_stock(stock.into());
        lib.set_stock_note(note.into());
        let default = default_date(&roll);
        lib.set_shot_date_set(film.shot_date.is_some());
        lib.set_shot_date_default(default.is_some());
        let key = format!("roll {} {:?} {:?} {:?}", film.id, film.shot_date, film.camera_body, default);
        if s.org.info_loaded.as_deref() != Some(key.as_str()) {
            let [m, d, y] = date_fields(&roll);
            lib.set_month(m.into());
            lib.set_day(d.into());
            lib.set_year(y.into());
            lib.set_camera_body(film.camera_body.clone().unwrap_or_default().into());
            s.org.info_loaded = Some(key);
        }
    }

    /// `updateRoll`: a folder's roll is its marker; a roll its sidecars name is written on each
    /// member as one gallery step.
    pub(in crate::app) fn update_roll(self: &Rc<Self>, label: &str, mutate: impl FnOnce(&mut Film)) {
        let Some(roll) = self.state.borrow().roll() else { return };
        let mut film = roll.film.clone();
        mutate(&mut film);
        if film != roll.film {
            match &roll.marker {
                Some(folder) => {
                    match oni_library::folders::set_roll(Some(&film), folder) {
                        Ok(()) => self.roll_written(folder, Some(film.clone())),
                        Err(e) => self.told(tr("Something Went Wrong"), &tr!("The roll could not be written in its folder: {error}", error = oni_library::refusal::cause(&e))),
                    }
                    self.lay_archive();
                }
                None => {
                    let paths = roll.paths.clone();
                    let label = label.to_string();
                    self.gesture(&paths, |store| store.set_film(&paths, Some(film), &label));
                }
            }
        }
        self.state.borrow_mut().org.info_loaded = None;
        self.publish_info();
    }

    /// Which roll or project the sidebar shows, so a draft typed for one never lands on another.
    fn info_subject(&self) -> Option<String> {
        let s = self.state.borrow();
        if s.org.projects_surface {
            return s.project_group().map(|g| format!("project {}", g.id));
        }
        s.roll().filter(|_| !s.trash_shown()).map(|r| format!("roll {}", r.film.id))
    }

    pub(in crate::app) fn title_edited(&self, draft: &str) {
        let subject = self.info_subject().filter(|_| !draft.is_empty());
        self.state.borrow_mut().org.title_draft = subject.map(|on| (on, draft.to_string()));
    }

    /// What the sidebar's fields hold and never committed, written before a scene hides them: a
    /// field destroyed with the focus never says it was left. One step for the roll's fields.
    pub(in crate::app) fn commit_info_drafts(self: &Rc<Self>) {
        // A disk gesture under way refuses the roll's write, so the drafts stay in their fields.
        if self.state.borrow().org.disk_busy.is_some() {
            return;
        }
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let subject = self.info_subject();
        let draft = self.state.borrow_mut().org.title_draft.take();
        let title = draft.filter(|(on, _)| Some(on) == subject.as_ref()).map(|(_, t)| t);
        if self.state.borrow().org.projects_surface {
            if lib.get_info_kind() == 2 {
                self.description_committed(&lib.get_description());
            }
            if let Some(title) = title {
                self.title_committed(&title);
            }
            return;
        }
        let Some(roll) = self.state.borrow().roll().filter(|_| lib.get_info_kind() == 1) else { return };
        let mut film = roll.film.clone();
        if let Some(name) = title.and_then(|t| picking::named(&t, &film.name)) {
            film.name = name;
        }
        let body = lib.get_camera_body().trim().to_string();
        if body != film.camera_body.clone().unwrap_or_default() {
            film.camera_body = (!body.is_empty()).then_some(body);
        }
        let typed = [lib.get_month(), lib.get_day(), lib.get_year()].map(|t| t.to_string());
        if typed != date_fields(&roll)
            && let Some(date) = typed_date(&typed[0], &typed[1], &typed[2])
        {
            film.shot_date = Some(date);
        }
        if film == roll.film {
            return;
        }
        let label = if film.name != roll.film.name {
            tr("Rename Roll")
        } else if film.camera_body != roll.film.camera_body {
            tr("Set Camera Body")
        } else {
            tr("Set Shot Date")
        };
        self.update_roll(label, |f| *f = film);
    }

    pub(in crate::app) fn title_committed(self: &Rc<Self>, draft: &str) {
        self.state.borrow_mut().org.title_draft = None;
        if self.state.borrow().org.projects_surface {
            let renamed = {
                let mut s = self.state.borrow_mut();
                let Some(group) = s.project_group() else { return };
                let Some(name) = picking::named(draft, &group.project.name) else { return };
                project::rename(&mut s.org.projects, group.id, &name)
            };
            if renamed {
                self.save_projects();
                self.lay_grid();
            }
            return;
        }
        let Some(roll) = self.state.borrow().roll() else { return };
        let Some(name) = picking::named(draft, &roll.film.name) else { return };
        self.update_roll(tr("Rename Roll"), |f| f.name = name);
    }

    /// `DateFields.commit`: month and year must read as numbers, the day falls back on the first.
    pub(in crate::app) fn shot_date_committed(self: &Rc<Self>, month: &str, day: &str, year: &str) {
        match typed_date(month, day, year) {
            Some(date) => self.update_roll(tr("Set Shot Date"), |f| f.shot_date = Some(date)),
            None => {
                self.state.borrow_mut().org.info_loaded = None;
                self.publish_info();
            }
        }
    }

    pub(in crate::app) fn shot_date_cleared(self: &Rc<Self>) {
        self.update_roll(tr("Set Shot Date"), |f| f.shot_date = None);
    }

    /// Written on Return or when the focus leaves: each keystroke would rewrite the whole roll.
    pub(in crate::app) fn camera_body_committed(self: &Rc<Self>, draft: &str) {
        let wanted = draft.trim().to_string();
        let Some(roll) = self.state.borrow().roll() else { return };
        if wanted == roll.film.camera_body.clone().unwrap_or_default() {
            return;
        }
        self.update_roll(tr("Set Camera Body"), |f| f.camera_body = (!wanted.is_empty()).then_some(wanted));
    }

    pub(in crate::app) fn description_committed(self: &Rc<Self>, text: &str) {
        let set = {
            let mut s = self.state.borrow_mut();
            let Some(group) = s.project_group() else { return };
            if group.project.description == text {
                return;
            }
            project::set_description(&mut s.org.projects, group.id, text)
        };
        if set {
            self.save_projects();
        }
    }
}

/// The first photo's capture, shown while the roll has no date of its own.
fn default_date(roll: &Roll) -> Option<Date> {
    if roll.film.shot_date.is_none() { roll.first.as_ref().and_then(|e| e.captured_at) } else { None }
}

/// The month, day and year the date fields show for `roll`.
fn date_fields(roll: &Roll) -> [String; 3] {
    let (y, m, d) = oni_library::info::ymd(roll.film.shot_date.or(default_date(roll)).unwrap_or_else(Date::now));
    [m.to_string(), d.to_string(), y.to_string()]
}

/// The date the fields spell: month and year must read as numbers, the day falls back on the first.
fn typed_date(month: &str, day: &str, year: &str) -> Option<Date> {
    match (month.trim().parse::<u32>(), year.trim().parse::<i32>()) {
        (Ok(m), Ok(y)) => oni_library::info::made(y, m, day.trim().parse().unwrap_or(1)),
        _ => None,
    }
}
