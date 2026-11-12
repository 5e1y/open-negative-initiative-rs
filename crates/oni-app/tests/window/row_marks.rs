//! The rows on the headless window: their end marks in one column on the right edge, a roll's stock
//! canister (Foma 401's though no longer offered), a selected row flat where the tab bar keeps its relief.

use i_slint_backend_testing::{AccessibleRole as Role, ElementRoot};
use oni_app::app::{Options, Shell};
use oni_app::{App, AppSettings, FolderLine, Library, Navigator, Stocks};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Film, Id, PipelineSettings};
use slint::platform::WindowEvent;
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::{self, Placed};

/// `RowStyle.mark`, the room `Space.s` between two marks' columns and `RowStyle.padding`, in points.
const MARK: f32 = 12.8;
const MARKS_GAP: f32 = 8.0;
const PADDING: f32 = 8.0;
const SCALE: f32 = 2.0;

/// Whether the box, in points, shows the favourite's gold (`DS.tint-favourite`) as the window's 16-bit
/// pixels keep it.
fn golden(px: &(usize, Vec<Rgb565Pixel>), r: Placed) -> bool {
    const GOLD: u32 = 0xf2c94c;
    let near = |v: u16| {
        let (red, green, blue) = ((v >> 11) as i32, ((v >> 5) & 0x3f) as i32, (v & 0x1f) as i32);
        (red - (GOLD >> 19 & 0x1f) as i32).abs() <= 1 && (green - (GOLD >> 10 & 0x3f) as i32).abs() <= 2 && (blue - (GOLD >> 3 & 0x1f) as i32).abs() <= 1
    };
    let (x0, y0, x1, y1) = ((r.x * SCALE) as usize, (r.y * SCALE) as usize, ((r.x + r.width) * SCALE) as usize, ((r.y + r.height) * SCALE) as usize);
    (y0..y1).any(|y| (x0..x1).any(|x| near(px.1[y * px.0 + x].0)))
}
/// The mean gap, per channel out of 255, between an artwork and its pixels drawn: RGB565's steps.
const DRAWN_AS: f32 = 6.0;

/// A photo of `folder`, measured with its roll or never touched, which leaves the roll to prepare.
fn photo(folder: &Path, measured: bool) {
    let path = folder.join("A.tif");
    common::linear_tiff(&path, 64, 48, 0.5);
    if measured {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
        s.roll_reference = Some(RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
        sidecar::write(&s, &path, WriteOptions::default()).unwrap();
    }
}

fn rgb(p: Rgb565Pixel) -> [f32; 3] {
    let v = p.0;
    [((v >> 11) & 0x1f) as f32 * 255.0 / 31.0, ((v >> 5) & 0x3f) as f32 * 255.0 / 63.0, (v & 0x1f) as f32 * 255.0 / 31.0]
}

/// How far the row's drawn pixels lie from `id`'s artwork at its best place in the row's leading
/// room: the mean channel gap over the artwork's opaque pixels.
fn canister_gap(drawn: &(usize, Vec<Rgb565Pixel>), row: &Placed, id: &str) -> f32 {
    let (w, px) = drawn;
    let art = oni_app::stocks::row_icon(id, (22.0 * SCALE) as u32).unwrap().to_rgba8().unwrap();
    let (aw, ah) = (art.width() as usize, art.height() as usize);
    let opaque: Vec<(usize, usize, [f32; 3])> = (0..ah)
        .flat_map(|y| (0..aw).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let p = art.as_slice()[y * aw + x];
            (p.a == 255).then(|| (x, y, [p.r as f32, p.g as f32, p.b as f32]))
        })
        .collect();
    let (left, top) = ((row.x * SCALE) as usize, (row.y * SCALE) as usize);
    let (across, down) = ((120.0 * SCALE) as usize, (row.height * SCALE) as usize - ah);
    assert!((top + down + ah) * w <= px.len(), "the row lies inside the window: {row:?}");
    let mut best = f32::MAX;
    for oy in top..=top + down {
        for ox in left..left + across {
            let gap: f32 = opaque
                .iter()
                .map(|(x, y, a)| {
                    let d = rgb(px[(oy + y) * w + ox + x]);
                    (0..3).map(|c| (d[c] - a[c]).abs()).sum::<f32>() / 3.0
                })
                .sum::<f32>()
                / opaque.len() as f32;
            best = best.min(gap);
        }
    }
    best
}

/// Every element inheriting `Outward`, a raised surface with its shadow, where the layout put it.
fn raised(window: &impl ComponentHandle) -> Vec<Placed> {
    window
        .root_element()
        .query_descendants()
        .match_inherits("Outward")
        .find_all()
        .into_iter()
        .filter(|e| e.size().width > 0.0)
        .map(|e| {
            let (at, size) = (e.absolute_position(), e.size());
            Placed { x: at.x, y: at.y, width: size.width, height: size.height }
        })
        .collect()
}

/// Whether `a` lies on `b`, within a few points of its edges: a surface of its own, not one round it.
fn lies_on(a: &Placed, b: &Placed) -> bool {
    const SLACK: f32 = 4.0;
    a.x >= b.x - SLACK && a.y >= b.y - SLACK && a.x + a.width <= b.x + b.width + SLACK && a.y + a.height <= b.y + b.height + SLACK
}

/// A row's band read on a column `x` of it bare of words and marks, against the ground beside the row
/// at the same height: lit apart from it, one shade from its top to its foot, and nothing cast under it.
fn flat(drawn: &(usize, Vec<Rgb565Pixel>), row: &Placed, x: f32) -> (bool, bool, bool) {
    let (w, px) = drawn;
    let at = |x: f32, y: f32| rgb(px[(y * SCALE) as usize * w + (x * SCALE) as usize]);
    let beside = |y: f32| at(row.x - 4.0, y);
    let (top, middle, foot) = (at(x, row.y + 1.0), at(x, row.y + row.height / 2.0), at(x, row.y + row.height - 1.5));
    let under = row.y + row.height + 1.5;
    (middle != beside(row.y + row.height / 2.0), top == middle && middle == foot, at(x, under) == beside(under))
}

pub(crate) fn the_rows_lay_their_marks_on_one_edge_and_draw_their_rolls_canister() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the rows' marks are not exercised"),
    };
    let made = std::env::temp_dir().join(format!("oni-app-row-marks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&made);
    std::fs::create_dir_all(&made).unwrap();
    let root = PathBuf::from(oni_library::paths::canonical(&made));
    let (home, volumes) = (root.join("Home"), root.join("Volumes"));
    std::fs::create_dir_all(&volumes).unwrap();
    let scans = home.join("Scans");
    let long = "A roll whose name runs on well past the column";
    let roll = |folder: &Path, stock: Option<&str>| {
        let mut film = Film::new(&oni_library::paths::name(&folder.to_string_lossy()));
        film.stock = stock.map(str::to_owned);
        oni_settings::film::roll_folder::set_roll(Some(&film), folder).unwrap();
    };
    for (folder, stock, measured) in [
        (scans.join("Portra"), Some("kodak-portra"), false),
        (scans.join("Bare"), None, true),
        (scans.join("Fomapan"), Some("foma-fomapan401"), true),
        (scans.join("Deep").join(long), None, false),
    ] {
        photo(&folder, measured);
        roll(&folder, stock);
    }
    let p = |path: &Path| oni_library::paths::normal(&path.to_string_lossy()).into_owned();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let computer = oni_library::navigator::Computer { home: p(&home), volumes: p(&volumes) };
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(slint::PhysicalSize::new(2560, 1600));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let n = w.global::<Navigator>();
    let lines = || -> Vec<FolderLine> { n.get_rows().iter().collect() };
    let line = |title: &str| lines().into_iter().find(|l| l.title == title);
    let at = |title: &str| lines().iter().position(|l| l.title == title).unwrap() as i32;
    shell.add(&scans);
    rig.until("Scans read", Duration::from_secs(30), || ctl.state.borrow().read_done() && line("Scans").is_some_and(|l| l.opened));
    n.invoke_row_folded(at("Scans"));
    rig.until("Deep listed", Duration::from_secs(10), || line("Deep").is_some());
    n.invoke_row_folded(at("Deep"));
    rig.until("every roll listed and marked", Duration::from_secs(10), || {
        line(long).is_some_and(|l| l.unprepared && l.kind == 1) && line("Portra").is_some_and(|l| l.unprepared) && line("Fomapan").is_some_and(|l| l.kind == 1)
    });
    n.invoke_star_clicked(at("Portra"));
    n.invoke_star_clicked(at("Bare"));
    rig.until("two favourites", Duration::from_secs(10), || line("Portra").is_some_and(|l| l.favourite) && line("Bare").is_some_and(|l| l.favourite));
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(1200.0, 700.0) });
    rig.settle();

    // The navigator's own: a roll's name heads its band in the gallery too.
    let leftmost = |role: Role, label: &str| {
        let found = common::placed(w, role, label);
        found.iter().copied().min_by(|a, b| a.x.total_cmp(&b.x)).unwrap_or_else(|| panic!("{label:?} shown: {found:?}"))
    };
    let row = |title: &str| leftmost(Role::ListItem, title);
    let on = |marks: &[Placed], title: &str| marks.iter().copied().filter(|m| row(title).level_with(m)).collect::<Vec<_>>();
    let centre = |m: &Placed| m.x + m.width / 2.0;
    let warnings = common::placed(w, Role::Image, "Not prepared yet");
    let stars = common::placed(w, Role::Button, "Remove from the favourites");
    println!("shot {}", rig.shot("row-marks").display());
    let (portra_warning, long_warning) = (on(&warnings, "Portra"), on(&warnings, long));
    let (portra_star, bare_star) = (on(&stars, "Portra"), on(&stars, "Bare"));
    assert!(portra_warning.len() == 1 && long_warning.len() == 1, "each roll to prepare shows one warning: {warnings:?}");
    assert!(portra_star.len() == 1 && bare_star.len() == 1, "each favourite shows its star: {stars:?}");
    // The warnings share one column, with a star beside it or none, a row deeper and its title cut.
    assert_eq!(portra_warning[0].x, long_warning[0].x, "one column of warnings: {portra_warning:?} {long_warning:?}");
    assert_eq!(portra_warning[0].width, MARK, "a warning is a mark wide");
    // The stars share one column, a mark wide on the row's right edge, whether a warning stands beside it or not.
    assert_eq!(centre(&portra_star[0]), centre(&bare_star[0]), "one column of stars");
    let edge = row("Portra").x + row("Portra").width - PADDING;
    assert!((centre(&portra_star[0]) + MARK / 2.0 - edge).abs() < 0.01, "the star's column ends on the row's edge: {} {edge}", centre(&portra_star[0]));
    assert!((centre(&portra_star[0]) - centre(&portra_warning[0]) - (MARK + MARKS_GAP)).abs() < 0.01, "the warning's column, a mark and its gap before the star's");
    // Twins: the titles start at their own depth's indent, so the x read is each element's own, and
    // two columns stand apart.
    let head = line(long).unwrap().head.to_string();
    assert!(leftmost(Role::Text, &head).x > leftmost(Role::Text, "Portra").x, "the deeper row's title starts further in");
    assert_ne!(centre(&portra_star[0]), centre(&portra_warning[0]), "the star's column is not the warning's");

    // The hand on the long roll's row shows its bare star in the stars' column; its warning stays put.
    let hovered = row(long).middle();
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(hovered.0 - 60.0, hovered.1) });
    rig.pump();
    let offered = on(&common::placed(w, Role::Button, "Add to the favourites"), long);
    assert_eq!(offered.len(), 1, "the hovered row offers its star");
    assert_eq!(centre(&offered[0]), centre(&portra_star[0]), "the offered star stands in the stars' column");
    assert_eq!(on(&common::placed(w, Role::Image, "Not prepared yet"), long)[0].x, long_warning[0].x, "the star never pushes the warning");
    println!("shot {}", rig.shot("row-marks-hovered").display());
    // A star under the hand shows what its press makes: the offered one lights gold, a favourite's goes out.
    let point = |star: Placed| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(star.x + star.width / 2.0, star.y + star.height / 2.0) });
        rig.pump();
        golden(&rig.drawn(), star)
    };
    assert!(!golden(&rig.drawn(), offered[0]), "twin: the offered star, the hand beside it, is not gold");
    assert!(point(offered[0]), "under the hand, the offered star lights gold");
    println!("shot {}", rig.shot("row-marks-star-offered").display());
    assert!(golden(&rig.drawn(), portra_star[0]), "twin: a favourite's star, the hand away from it, is gold");
    assert!(!point(portra_star[0]), "under the hand, a favourite's star goes out");
    println!("shot {}", rig.shot("row-marks-star-removing").display());
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(1200.0, 700.0) });
    rig.settle();

    // Each roll draws its stock's canister, the generic negative's with none picked, Foma 401's too.
    let drawn = rig.drawn();
    for (title, stock, other) in [
        ("Portra", "kodak-portra", "generic-color-negative"),
        ("Bare", "generic-color-negative", "kodak-portra"),
        ("Fomapan", "foma-fomapan401", "generic-black-and-white"),
        (long, "generic-color-negative", "ilford-hp5"),
    ] {
        let (own, wrong) = (canister_gap(&drawn, &row(title), stock), canister_gap(&drawn, &row(title), other));
        println!("{title}: {stock} {own:.2}, {other} {wrong:.2}");
        assert!(own < DRAWN_AS, "{title} draws {stock}'s canister: gap {own:.1}");
        // Twin: another stock's canister is not what the row draws.
        assert!(wrong > DRAWN_AS, "{title} draws no {other}: gap {wrong:.1}");
    }

    // The opened folder's row is selected flat: no raised surface on it, one shade over the ground.
    let opened = row("Scans");
    let tabs = common::the(w, Role::Tab, "Archive");
    let surfaces = raised(w);
    assert!(!surfaces.iter().any(|s| lies_on(s, &opened)), "no raised surface under the selected row: {surfaces:?}");
    // Twin: the tab bar keeps its raised pill, which the same search finds.
    assert!(surfaces.iter().any(|s| lies_on(s, &tabs)), "the tab bar's pill is raised: {surfaces:?} {tabs:?}");
    let bare = opened.x + opened.width - PADDING - MARK - MARKS_GAP - 4.0;
    assert_eq!(flat(&drawn, &opened, bare), (true, true, true), "the selected row's band: lit, flat and casting nothing");
    // Twin: a row neither selected nor under the hand shows no band at all.
    assert!(!flat(&drawn, &row("Bare"), bare).0, "a row at rest lays no band");

    // Foma 401 still reads on its roll, its name and canister shown, while the picker offers it no more.
    let lib = w.global::<Library>();
    let fomapan = p(&scans.join("Fomapan").join("A.tif"));
    let index = ctl.state.borrow().order().iter().position(|path| *path == fomapan).expect("Fomapan's photo is shown") as i32;
    lib.invoke_tile_clicked(index, false, false);
    rig.pump();
    assert_eq!(lib.get_stock().as_str(), "Foma Fomapan 401", "the roll still names its stock");
    assert!(lib.get_stock_has_artwork(), "and shows its canister");
    lib.invoke_select_roll_type();
    rig.pump();
    let stocks = w.global::<Stocks>();
    let brands = stocks.get_brands();
    let names: Vec<String> = brands
        .iter()
        .flat_map(|b| b.rows.iter().collect::<Vec<_>>())
        .flat_map(|r| r.cells.iter().collect::<Vec<_>>())
        .map(|c| c.name.to_string())
        .collect();
    assert!(!names.iter().any(|n| n == "Foma Fomapan 401"), "the picker offers no Foma 401: {names:?}");
    // Twin: Foma's other stocks stay offered.
    assert!(names.iter().any(|n| n == "Foma Fomapan 400"), "{names:?}");
    rig.key("\u{1b}");
    assert!(!stocks.get_shown());

    // Beside the photo, the archive's tree draws its rolls' canisters as the navigator above it does.
    lib.invoke_tile_opened(index);
    rig.until("the photo opened", Duration::from_secs(30), || !w.global::<App>().get_gallery());
    rig.settle();
    let drawn = rig.drawn();
    println!("shot {}", rig.shot("row-marks-tree").display());
    let fomapans = common::placed(w, Role::ListItem, "Fomapan");
    assert!(fomapans.len() == 2, "Fomapan listed by the navigator and the tree: {fomapans:?}");
    for found in &fomapans {
        assert!(canister_gap(&drawn, found, "foma-fomapan401") < DRAWN_AS, "each Fomapan line draws Foma 401's canister: {found:?}");
        // Twin: neither draws the generic black and white's.
        assert!(canister_gap(&drawn, found, "generic-black-and-white") > DRAWN_AS);
    }

    // Settings' sections: the one shown selected flat as well.
    let settings = w.global::<AppSettings>();
    settings.set_section(1);
    settings.set_shown(true);
    rig.settle();
    let editor = common::the(w, Role::ListItem, "Editor");
    let frontier = common::the(w, Role::ListItem, "Frontier Look");
    let drawn = rig.drawn();
    println!("shot {}", rig.shot("row-marks-settings").display());
    assert!(!raised(w).iter().any(|s| lies_on(s, &editor)), "no raised surface under the section shown");
    let side = editor.x + editor.width - PADDING;
    assert_eq!(flat(&drawn, &editor, side), (true, true, true), "the section shown: lit, flat and casting nothing");
    // Twin: the section under it, not shown, lays no band.
    assert!(!flat(&drawn, &frontier, side).0, "a section at rest lays no band");
    settings.set_shown(false);

    // Back in the gallery, Bare's photo trashed: the ONI Trash's line for Bare keeps its count on the
    // edge while the hand shows its Restore key, which stands before the count.
    ctl.act(oni_app::Act::ToggleMode);
    rig.until("the gallery back", Duration::from_secs(10), || w.global::<App>().get_gallery());
    let bare = p(&scans.join("Bare").join("A.tif"));
    let index = ctl.state.borrow().order().iter().position(|path| *path == bare).expect("Bare's photo is shown") as i32;
    lib.invoke_tile_clicked(index, false, false);
    rig.pump();
    ctl.act(oni_app::Act::Remove);
    rig.landed(&ctl);
    lib.invoke_trash_row_folded(0);
    rig.until("the trash's folder listed", Duration::from_secs(10), || lib.get_trash_rows().row_count() == 2);
    rig.settle();
    let trashed = common::placed(w, Role::ListItem, "Bare").into_iter().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
    let count = || common::placed(w, Role::Text, "1").into_iter().find(|t| trashed.level_with(t) && t.x > trashed.x).expect("the trash's line counts its photo");
    let at_rest = count();
    assert!(common::placed(w, Role::Button, "Restore").iter().all(|k| !trashed.level_with(k)), "no key at rest");
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(trashed.x + 40.0, trashed.middle().1) });
    rig.pump();
    println!("shot {}", rig.shot("row-marks-trash").display());
    let key = common::placed(w, Role::Button, "Restore").into_iter().find(|k| trashed.level_with(k)).expect("the hovered line shows Restore");
    let hovered = count();
    assert_eq!(hovered.x + hovered.width, at_rest.x + at_rest.width, "the count keeps its edge under the hand");
    assert!((at_rest.x + at_rest.width - (trashed.x + trashed.width - PADDING)).abs() < 0.01, "on the row's right edge");
    // Twin: the key stands before the count, never after it.
    assert!(key.x + key.width <= hovered.x, "Restore {key:?} before the count {hovered:?}");
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&made);
}
