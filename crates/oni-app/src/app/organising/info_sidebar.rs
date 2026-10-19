//! The info sidebar: the roll's or the project's name, stock, date, body and description.

use super::*;

impl Ctl {
    pub(super) fn publish_info(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let mut s = self.state.borrow_mut();
        if s.org.projects_surface {
            let Some(group) = s.project_group() else {
                lib.set_info_kind(0);
                lib.set_info_empty("Select a project".into());
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
                .map(|e| s.thumbs.get(&e.fingerprint).unwrap_or_default())
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
            lib.set_info_empty("Select a roll".into());
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
            (None, Some(k), Some(m)) => (k.display_name.to_string(), format!("Default, from this roll's {} mode", m.label())),
            _ => (String::new(), "No stock set".to_string()),
        };
        let artwork = explicit.or(fallback).and_then(|k| crate::stocks::icon(k.icon_name));
        lib.set_stock_has_artwork(artwork.is_some());
        lib.set_stock_artwork(artwork.unwrap_or_default());
        lib.set_stock(stock.into());
        lib.set_stock_note(note.into());
        let default = if film.shot_date.is_none() { roll.first.as_ref().and_then(|e| e.captured_at) } else { None };
        lib.set_shot_date_set(film.shot_date.is_some());
        lib.set_shot_date_default(default.is_some());
        let key = format!("roll {} {:?} {:?} {:?}", film.id, film.shot_date, film.camera_body, default);
        if s.org.info_loaded.as_deref() != Some(key.as_str()) {
            let (y, m, d) = oni_library::info::ymd(film.shot_date.or(default).unwrap_or_else(Date::now));
            lib.set_month(m.to_string().into());
            lib.set_day(d.to_string().into());
            lib.set_year(y.to_string().into());
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
                    if let Err(e) = oni_library::folders::set_roll(Some(&film), folder) {
                        self.say(&format!("The roll could not be written in its folder: {e}"));
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

    pub(in crate::app) fn title_committed(self: &Rc<Self>, draft: &str) {
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
        self.update_roll("Rename Roll", |f| f.name = name);
    }

    /// `DateFields.commit`: month and year must read as numbers, the day falls back on the first.
    pub(in crate::app) fn shot_date_committed(self: &Rc<Self>, month: &str, day: &str, year: &str) {
        let made = match (month.trim().parse::<u32>(), year.trim().parse::<i32>()) {
            (Ok(m), Ok(y)) => oni_library::info::made(y, m, day.trim().parse().unwrap_or(1)),
            _ => None,
        };
        match made {
            Some(date) => self.update_roll("Set Shot Date", |f| f.shot_date = Some(date)),
            None => {
                self.state.borrow_mut().org.info_loaded = None;
                self.publish_info();
            }
        }
    }

    pub(in crate::app) fn shot_date_cleared(self: &Rc<Self>) {
        self.update_roll("Set Shot Date", |f| f.shot_date = None);
    }

    /// Written on Return or when the focus leaves: each keystroke would rewrite the whole roll.
    pub(in crate::app) fn camera_body_committed(self: &Rc<Self>, draft: &str) {
        let wanted = draft.trim().to_string();
        let Some(roll) = self.state.borrow().roll() else { return };
        if wanted == roll.film.camera_body.clone().unwrap_or_default() {
            return;
        }
        self.update_roll("Set Camera Body", |f| f.camera_body = (!wanted.is_empty()).then_some(wanted));
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
