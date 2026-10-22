//! `Shell`: the window and its controller, as `main` and the checks build and drive them.

use super::*;

impl Shell {
    /// The window over `engine`, whose device the canvas's texture must share when Slint draws on wgpu.
    pub fn new(store: Box<dyn Settings>, engine: Shared, options: Options) -> Result<Shell, slint::PlatformError> {
        let window = MainWindow::new()?;
        let caches = engine.lock().ok().and_then(|e| e.caches.clone());
        crate::measuring::set_folders(&options.support, caches);
        let worker = Worker::new(Box::new(engine.clone()), |done| with_shell(move |c| c.deliver(done)));
        let (device, queue) = engine.lock().map_err(slint::PlatformError::Other)?.device();
        engine.adopt().map_err(slint::PlatformError::Other)?;
        let index_path = LibraryIndex::default_path(&options.caches);
        let painter: Painter = Arc::default();
        let previews: Arc<Mutex<Previews>> = Arc::default();
        let hands: Arc<AtomicUsize> = Arc::default();
        let generation: Arc<AtomicU64> = Arc::default();
        paint(painter.clone(), engine.clone(), previews.clone(), hands.clone(), generation.clone(), options.support.clone(), options.screen);
        let disk = read_disk(options.support.clone(), options.screen);
        let keep = Preferences::load(&options.support).bool(preferences::KEEP_ZOOM).unwrap_or(false);
        crate::viewport::KEEP_ZOOM.store(keep, Ordering::Relaxed);
        let state = State {
            store,
            screen: options.screen,
            archive: None,
            order: Vec::new(),
            fingerprints: HashMap::new(),
            opened: None,
            preferences: Preferences::load(&options.support),
            index: LibraryIndex::open(&index_path),
            index_path,
            survey: Survey::default(),
            generation,
            read_done: false,
            opening: None,
            filter: None,
            painter,
            disk,
            asked: HashSet::new(),
            painted: (0, 0),
            creating: (0, 0),
            failed: HashSet::new(),
            drawing: 0,
            rows: Vec::new(),
            pending: None,
            muted: Preferences::load(&options.support).bool(preferences::SOUND_MUTED).unwrap_or(true),
            open: None,
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
                .map_or(gallery::COLUMN_STEPS[1], |n| gallery::column_step(n.max(1) as usize, 0)),
            scroll: 0.0,
            grid: (0.0, 0.0),
            layout: Layout::new(&[], 6, 0.0, Metrics::GALLERY),
            visible: 0..0,
            thumbs: Memory::new(thumbnails::MEMORY_LIMIT),
            negative: None,
            previews,
            hands,
            slides: Field::ALL.iter().map(|_| Slide::default()).collect(),
            rendered: 0,
            requested: 0,
            histogram: None,
            histogram_dirty: false,
            inputs: None,
            inputs_drawn: None,
            last_change: None,
            published_revision: 0,
            reading: None,
            quitting: false,
            stock_target: None,
            masks: crate::flat_fields::entries(&options.support),
            masking: masks::Masking::new(options.mask_picker, options.show_folder),
            status: String::new(),
            brush: Preferences::load(&options.support).value(preferences::BRUSH_RADIUS).and_then(serde_json::Value::as_f64).map_or(oni_settings::frame::Correction::DEFAULT_RADIUS as f32, |r| r as f32),
            // No rendering notifier: Slint flushes femtovg before calling it, which empties femtovg's
            // pipeline cache, so every frame recompiled its pipelines (8 ms of the interface thread on Metal).
            gpu: Gpu {
                kernel: display::Gpu::new(&device),
                device,
                queue,
                target: None,
                before: None,
                current: None,
                reads_back: !options.canvas,
                uncounted: None,
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
            full: None,
            building_full: None,
            full_failed: HashSet::new(),
            loupe: loupe::Held::default(),
            together: None,
            healing: HashSet::new(),
            patched: HashSet::new(),
            unhealable: HashSet::new(),
            full_size: None,
            requested_view: None,
            hand: None,
            requested_live: None,
            placed: None,
            grab: None,
            pinch: 1.0,
            split: None,
            framing_memory: None,
            frame_gesture: None,
            frame_cancelled: false,
            scrolling: false,
            straightening: None,
            route: crate::tabs::Route::default(),
            painting_stroke: Default::default(),
            numbers: HashMap::new(),
            next_number: 1,
            fallback: std::collections::HashSet::new(),
            sources: HashMap::new(),
            brush_start: None,
            brush_pressed_at: None,
            window_board: None,
            rename_request: None,
            tree_renaming: None,
            pasteboard: options.pasteboard,
            board_seen: None,
            befores: 0,
            screen_gamut: false,
            before_placed: None,
            support: options.support.clone(),
            phone: None,
            picker: options.picker,
            discard: options.discard,
            reporter: options.reporter,
            news_kept: HashSet::new(),
            report_round: 0,
            console: None,
            nav: navigator::Nav::new(options.computer),
            prepare: prepare::Roll::new(options.require_roll),
            making_sheet: None,
            updates: updates::Updates::new(options.updates),
            year: year::Year::default(),
            transition: transition::Transition::new(options.transition, &options.support),
        };
        let ctl = Rc::new(Ctl {
            window: window.as_weak(),
            state: RefCell::new(state),
            engine,
            worker,
            trailing: Field::ALL.iter().map(|_| Timer::default()).collect(),
            arrivals: Field::ALL.iter().map(|_| Timer::default()).collect(),
            pulse: Timer::default(),
            histogram_watch: Timer::default(),
            settle: Timer::default(),
            stripes: Timer::default(),
            scroll_idle: Timer::default(),
            editors: crate::editors::Shell::default(),
            refresher: Timer::default(),
            writer: Timer::default(),
            export: RefCell::new(export_card::Card::new(&options.support)),
        });
        SHELL.with(|s| *s.borrow_mut() = Rc::downgrade(&ctl));
        Ctl::wire(&ctl, &window);
        Ctl::wire_export(&ctl, &window);
        ctl.attach_export();
        ctl.restore_gamut();
        // The app opens on the gallery; nothing opens in the editor until a hand asks.
        window.global::<App>().set_gallery(true);
        ctl.publish(true);
        ctl.publish_bar();
        ctl.start_updates();
        // A phone paired before finds the desktop listening again; nothing pairs until asked.
        if phone::has_trusted(&options.support) {
            ctl.start_phone();
        }
        Ok(Shell { window, ctl })
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
    pub fn last_writes(&self) -> Option<String> {
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
