//! `Shell`: the window and its controller, as `main` and the checks build and drive them.

use super::*;

impl Shell {
    /// The window over `engine`, whose device the canvas's texture must share when Slint draws on wgpu.
    pub fn new(store: EditorStore, engine: Shared, options: Options) -> Result<Shell, slint::PlatformError> {
        Self::with_window(MainWindow::new()?, store, engine, options)
    }

    /// `new` on a window already built: the shell stands, then rises, its steps run in a row.
    pub fn with_window(window: MainWindow, store: EditorStore, engine: Shared, options: Options) -> Result<Shell, slint::PlatformError> {
        let shell = Self::standing(window, store, engine, options)?;
        for step in shell.rising() {
            step.run();
        }
        Ok(shell)
    }

    /// The shell's state and controller on a window already built, each part's time in the log;
    /// `rising` then wires and lays out the window, a step each the starter names.
    pub fn standing(window: MainWindow, store: EditorStore, engine: Shared, options: Options) -> Result<Shell, slint::PlatformError> {
        let mut laps = crate::starter::Laps::new("Building the app");
        crate::keycaps::serve(&window);
        crate::slider_track::serve(&window);
        let caches = engine.lock().ok().and_then(|e| e.caches.clone());
        crate::measuring::set_folders(&options.support, caches);
        let worker = Worker::new(Box::new(engine.clone()), |done| with_shell(move |c| c.deliver(done)));
        laps.lap("the render worker");
        let (device, queue) = engine.lock().map_err(slint::PlatformError::Other)?.device();
        engine.adopt().map_err(slint::PlatformError::Other)?;
        laps.lap("the canvas's device");
        let index_path = LibraryIndex::default_path(&options.caches);
        let painter: Painter = Arc::default();
        let previews: Arc<Mutex<Previews>> = Arc::default();
        let hands: Arc<AtomicUsize> = Arc::default();
        paint(painter.clone(), engine.clone(), previews.clone(), hands.clone(), options.support.clone(), options.screen);
        let disk = read_disk(options.support.clone(), options.screen);
        laps.lap("the painter and the disk's state");
        let keep = Preferences::load(&options.support).bool(preferences::KEEP_ZOOM).unwrap_or(false);
        crate::viewport::KEEP_ZOOM.store(keep, Ordering::Relaxed);
        let whole = Preferences::load(&options.support).bool(preferences::FULL_DECODE).unwrap_or(false);
        crate::render::FULL_DECODE.store(whole, Ordering::Relaxed);
        crate::language::speak(crate::language::chosen(&Preferences::load(&options.support)));
        laps.lap("the preferences and the language");
        let index = LibraryIndex::open(&index_path);
        laps.lap("the library index");
        let kernel = display::Gpu::new(&device);
        laps.lap("the display kernel");
        let masks = crate::flat_fields::entries(&options.support);
        let nav = navigator::Nav::new(options.computer, options.identify);
        let export = export_card::Card::new(&options.support);
        laps.lap("the flat fields, the navigator and the export card");
        let state = State {
            store,
            screen: options.screen,
            archive: None,
            order: Vec::new(),
            fingerprints: HashMap::new(),
            folder: None,
            heading: None,
            preferences: Preferences::load(&options.support),
            index,
            index_path,
            reads: Default::default(),
            folder_turns: Default::default(),
            opening: None,
            filter: None,
            painter,
            disk,
            asked: HashSet::new(),
            painted: (0, 0),
            creating: (0, 0),
            failed: HashSet::new(),
            drawing: 0,
            pending: None,
            muted: Preferences::load(&options.support).bool(preferences::SOUND_MUTED).unwrap_or(true),
            frame: None,
            picks: Default::default(),
            trash_picks: Default::default(),
            tree: Default::default(),
            project_tree: Default::default(),
            tree_centred: None,
            reveal: Default::default(),
            shelf: Default::default(),
            batch: None,
            org: Default::default(),
            // A stored count off the ladder lands on its nearest rung.
            columns: Preferences::load(&options.support)
                .int(preferences::GALLERY_COLUMNS)
                .map_or(gallery::COLUMN_STEPS[2], |n| gallery::column_step(n.max(1) as usize, 0)),
            scroll: 0.0,
            grid: (0.0, 0.0),
            layout: Layout::new(&[], 6, 0.0, Metrics::GALLERY),
            visible: 0..0,
            thumbs: Memory::new(thumbnails::MEMORY_LIMIT),
            previews,
            hands,
            slides: Field::ALL.iter().map(|_| Slide::default()).collect(),
            arrivals: 0,
            histogram_dirty: false,
            last_change: None,
            published_revision: 0,
            quitting: false,
            stock_target: None,
            masks,
            masking: masks::Masking::new(options.mask_picker, options.show_folder),
            status: String::new(),
            brush: Preferences::load(&options.support).value(preferences::BRUSH_RADIUS).and_then(serde_json::Value::as_f64).map_or(oni_settings::frame::Correction::DEFAULT_RADIUS as f32, |r| r as f32),
            // No rendering notifier: Slint flushes femtovg before calling it, which empties femtovg's
            // pipeline cache, so every frame recompiled its pipelines (8 ms of the interface thread on Metal).
            gpu: Gpu {
                kernel,
                device,
                queue,
                target: None,
                before: None,
                current: None,
                reads_back: !options.canvas,
                seed: 0,
                screen: options.screen,
                counted_at: None,
            },
            renders: 0,
            opens: 0,
            opened_at: Instant::now(),
            notches: Notches::default(),
            moved_at: None,
            viewport: Viewport::default(),
            views: Notches::default(),
            building_full: None,
            full_failed: HashSet::new(),
            befores: 0,
            dust: dedusting::Dust { next_number: 1, sensitivity: Preferences::load(&options.support).value(preferences::DUST_SENSITIVITY).and_then(serde_json::Value::as_f64).map_or(0.0, |v| v as f32), ..Default::default() },
            loupe: loupe::Held::default(),
            together: None,
            grab: None,
            pinch: 1.0,
            framing_memory: None,
            scrolling: None,
            plot_down: None,
            route: crate::tabs::Route::default(),
            brush_start: None,
            brush_pressed_at: None,
            window_board: None,
            rename_request: None,
            tree_renaming: None,
            pasteboard: options.pasteboard,
            board_seen: None,
            screen_gamut: false,
            support: options.support.clone(),
            phone_offered: options.phone,
            phone: None,
            picker: options.picker,
            discard: options.discard,
            reporter: options.reporter,
            news_kept: HashSet::new(),
            report_round: 0,
            console: None,
            nav,
            prepare: prepare::Roll::new(options.require_roll),
            updates: updates::Updates::new(options.updates),
            year: year::Year::default(),
        };
        let ctl = Rc::new(Ctl {
            window: window.as_weak(),
            state: RefCell::new(state),
            engine,
            worker,
            trailing: Field::ALL.iter().map(|_| Timer::default()).collect(),
            arrival: Timer::default(),
            pulse: Timer::default(),
            histogram_watch: Timer::default(),
            settle: Timer::default(),
            stripes: Timer::default(),
            scroll_idle: Timer::default(),
            editors: crate::editors::Shell::default(),
            refresher: Timer::default(),
            writer: Timer::default(),
            export: RefCell::new(export),
        });
        SHELL.with(|s| *s.borrow_mut() = Rc::downgrade(&ctl));
        laps.lap("the controller");
        Ok(Shell { window, ctl })
    }

    /// The window wired, its colours, the gallery laid out, the ONI Trash counted, the updates and the
    /// phone: a step each, in the order the shell has always taken them.
    pub fn rising(&self) -> Vec<crate::starter::Step> {
        use crate::starter::step;
        use oni_i18n::marked;
        let (c, w) = (self.ctl.clone(), self.window.as_weak());
        vec![
            step(marked("Wiring the interface"), {
                let (c, w) = (c.clone(), w.clone());
                move || {
                    if let Some(window) = w.upgrade() {
                        Ctl::wire(&c, &window);
                        Ctl::wire_export(&c, &window);
                    }
                    c.attach_export();
                }
            }),
            step(marked("Matching the screen's colours"), {
                let c = c.clone();
                move || c.restore_gamut()
            }),
            step(marked("Laying out the gallery"), {
                let (c, w) = (c.clone(), w.clone());
                move || {
                    // The app opens on the gallery; nothing opens in the editor until a hand asks.
                    if let Some(window) = w.upgrade() {
                        window.global::<App>().set_gallery(true);
                        window.global::<App>().set_dev(!crate::updates::stamped());
                        window.global::<crate::Motion>().set_reduced(crate::system::reduce_motion());
                    }
                    c.publish(true);
                    c.publish_bar();
                }
            }),
            step(marked("Counting the ONI Trash"), {
                let c = c.clone();
                // The ONI Trash counts the whole library before any folder opens.
                move || {
                    c.read_library();
                    c.publish_trash_rows();
                }
            }),
            step(marked("Checking for updates"), {
                let c = c.clone();
                move || c.start_updates()
            }),
            step(marked("Reconnecting the paired phone"), move || {
                let (offered, support) = {
                    let s = c.state.borrow();
                    (s.phone_offered, s.support.clone())
                };
                if let Some(window) = w.upgrade() {
                    window.global::<Pairing>().set_available(offered);
                }
                // A phone paired before finds the desktop listening again; nothing pairs until asked.
                if offered && phone::has_trusted(&support) {
                    c.start_phone();
                }
            }),
        ]
    }

    /// `openFolder`: the gallery on the folder, read through the index behind, its thumbnails
    /// drawn by the pass; nothing opens in the editor until a hand asks.
    pub fn open_folder(&self, folder: &Path) {
        self.ctl.open_folder(folder);
    }

    /// `add(_:)`: a folder opens; a file opens its folder, then itself in the editor once found.
    pub fn add(&self, path: &Path) {
        self.ctl.add(&[path.to_path_buf()]);
    }

    /// The launch's folder: the one last opened, else the first root of `roots.json`, else none.
    pub fn open_remembered(&self) -> bool {
        self.ctl.open_remembered()
    }

    /// The opened folder's first photo in the editor once read, for the bench; a file named at
    /// launch keeps its own opening.
    pub fn open_first_when_read(&self) {
        self.ctl.state.borrow_mut().opening.get_or_insert_with(String::new);
    }

    /// `WelcomeSheet.isDue` at launch: shown once per edition, never under a bench.
    pub fn welcome_if_due(&self) {
        if self.ctl.welcome_due() {
            self.ctl.open_welcome();
        }
    }

    /// `offerWrapped()` at launch: December's summary, once a year, never under a bench.
    pub fn offer_wrapped(&self) {
        self.ctl.offer_wrapped_when_read();
    }

    /// The open frame's sidecar written as the app ends, whatever ended it; the failure, if any.
    pub fn flush(&self) -> Option<String> {
        self.ctl.state.borrow_mut().store.flush()
    }

    /// What closing the window writes, for a quit that never closes it.
    pub fn last_writes(&self) -> Option<String> {
        self.ctl.last_writes()
    }

    /// The session's notch renders, for the line printed on exit.
    pub fn summary(&self) -> String {
        self.ctl.state.borrow().notches.summary(0, "session")
    }
}

impl Ctl {
    /// The roll's draft, then the open frame's settings, as closing the window writes them.
    pub fn last_writes(self: &Rc<Self>) -> Option<String> {
        // A frame's move under way ends whole before the process does.
        gestures::wait_for_disk();
        // An export's frame under way lands whole, ledger included.
        self.export.borrow_mut().writer.ended_whole();
        self.commit_info_drafts();
        self.flush_drafts();
        // The journal's thread writes what it was handed before the process ends.
        let _ = year::journal_in(&self.state.borrow().support).read();
        self.state.borrow_mut().store.flush()
    }
}

impl Drop for Shell {
    /// The phone link stops with its window: its port is free for the next, and the phones hear it close.
    fn drop(&mut self) {
        if let Ok(mut s) = self.ctl.state.try_borrow_mut() {
            s.phone = None;
        }
    }
}
