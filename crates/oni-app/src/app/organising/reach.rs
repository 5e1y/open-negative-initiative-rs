//! What is out of reach, drawn: each state's words for a folder's row, a tile and the gallery's
//! banner, a missing original's picture in grey, and a triplet's pass gone. Reads no disk.

use super::*;
use oni_library::absence;
use oni_library::navigator::Computer;
use oni_library::paths;
use oni_library::presence::{Away, Presence};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;

/// The greyed pictures kept at once; past it they are made again as they show.
const GREYS: usize = 256;

/// What the interface keeps of the photos out of reach beyond their state.
#[derive(Default)]
pub struct Seen {
    /// Each lead whose triplet lost a pass: the first gone, by channel (0 red) and name.
    pub gone_passes: HashMap<String, (usize, String)>,
    /// Each photo no reader of ONI takes, by its format's name in capitals (`HEIC`).
    pub unsupported: HashMap<String, String>,
    greys: Greys,
}

/// Each missing original's picture in grey, keyed by its fingerprint and the buffer it was made from.
#[derive(Default)]
struct Greys(RefCell<HashMap<String, (usize, Image)>>);

/// The picture a tile shows: its thumbnail, greyed when its original is out of reach, as
/// `TileLayers`' faded layer filters it to no saturation.
pub(in crate::app) fn shown(seen: &Seen, fingerprint: &str, picture: Image, missing: bool) -> Image {
    if !missing {
        return picture;
    }
    let Some(pixels) = picture.to_rgba8() else { return picture };
    let made_from = pixels.as_bytes().as_ptr() as usize;
    let mut held = seen.greys.0.borrow_mut();
    if let Some((from, grey)) = held.get(fingerprint)
        && *from == made_from
    {
        return grey.clone();
    }
    if held.len() >= GREYS {
        held.clear();
    }
    let grey = Image::from_rgba8(greyed(&pixels));
    held.insert(fingerprint.to_owned(), (made_from, grey.clone()));
    grey
}

/// `CIColorControls` at saturation 0: each pixel its Rec. 709 luma, its alpha kept.
fn greyed(pixels: &SharedPixelBuffer<Rgba8Pixel>) -> SharedPixelBuffer<Rgba8Pixel> {
    let mut out = pixels.clone();
    for p in out.make_mut_slice() {
        let luma = (0.2126 * f32::from(p.r) + 0.7152 * f32::from(p.g) + 0.0722 * f32::from(p.b)).round() as u8;
        (p.r, p.g, p.b) = (luma, luma, luma);
    }
    out
}

/// The interface's mark for a state.
pub(in crate::app) fn reach(presence: Presence) -> crate::Reach {
    match presence {
        Presence::Present => crate::Reach::Here,
        Presence::Offline(Away::Disconnected) => crate::Reach::Offline,
        Presence::Offline(Away::Denied) => crate::Reach::Denied,
        Presence::Missing => crate::Reach::Missing,
    }
}

/// The volume `path` lies on, by its name, when it may come and go.
fn volume(computer: &Computer, path: &str) -> Option<String> {
    absence::volume_root(computer, path).map(|root| paths::name(&root).trim_end_matches(['/', '\\']).to_string()).filter(|n| !n.is_empty())
}

/// A folder's row out of reach: its caption in words, then its tooltip; empty for one there.
pub(in crate::app) fn row_words(presence: Presence, path: &str, computer: &Computer) -> (String, String) {
    match presence {
        Presence::Present => (String::new(), String::new()),
        Presence::Offline(Away::Disconnected) => match volume(computer, path) {
            Some(disk) => (tr!("{disk} not connected", disk = disk), tr!("{path} is on “{disk}”, which is not connected.", path = path, disk = disk)),
            None => (tr("Not connected").into(), tr!("{path} is on a disk that is not connected.", path = path)),
        },
        Presence::Offline(Away::Denied) => (tr("Access denied").into(), tr!("ONI is refused access to {path}.", path = path)),
        Presence::Missing => (tr("Not found").into(), tr!("{path} is no longer there: moved, renamed or deleted outside ONI.", path = path)),
    }
}

/// A tile's tooltip: where its original was, and why it cannot be reached.
pub(in crate::app) fn tile_note(presence: Presence, path: &str, computer: &Computer) -> String {
    match presence {
        Presence::Present => String::new(),
        Presence::Offline(Away::Disconnected) => match volume(computer, path) {
            Some(disk) => tr!("Original on “{disk}”, which is not connected: {path}", disk = disk, path = path),
            None => tr!("Original on a disk not connected: {path}", path = path),
        },
        Presence::Offline(Away::Denied) => tr!("ONI is refused access to the original at {path}", path = path),
        Presence::Missing => tr!("Original not found at {path}", path = path),
    }
}

/// A triplet's lead marked by its pass gone: the seal's words, then its tooltip.
pub(in crate::app) fn pass_words(channel: usize, name: &str) -> (String, String) {
    match channel {
        0 => (tr("R MISSING").into(), tr!("Its red pass “{pass}” is missing beside it.", pass = name)),
        1 => (tr("G MISSING").into(), tr!("Its green pass “{pass}” is missing beside it.", pass = name)),
        _ => (tr("B MISSING").into(), tr!("Its blue pass “{pass}” is missing beside it.", pass = name)),
    }
}

/// The banner over an opened folder out of reach: its title, then what it says; `kept` when tiles
/// of it are still shown. `None` for a folder there.
pub(in crate::app) fn banner_words(presence: Presence, folder: &str, computer: &Computer, kept: bool) -> Option<(String, String)> {
    let name = paths::name(folder);
    let rest = if kept { tr("These are the last thumbnails ONI kept; nothing has been changed.") } else { tr("Nothing has been changed.") };
    Some(match presence {
        Presence::Present => return None,
        Presence::Offline(Away::Disconnected) => match volume(computer, folder) {
            Some(disk) => (tr!("{disk} Not Connected", disk = disk), tr!("“{name}” is on “{disk}”, which is not connected. {rest}", name = name, disk = disk, rest = rest)),
            None => (tr("Disk Not Connected").into(), tr!("“{name}” is on a disk that is not connected. {rest}", name = name, rest = rest)),
        },
        Presence::Offline(Away::Denied) => (
            tr("No Access").into(),
            tr!("ONI is refused access to “{name}”: allow it to read the folder in the system's privacy settings. {rest}", name = name, rest = rest),
        ),
        Presence::Missing => (
            tr("Folder Not Found").into(),
            tr!(
                "“{name}” is no longer in “{parent}”: moved, renamed or deleted outside ONI. Its settings travel with its files.",
                name = name,
                parent = paths::name(&paths::parent(folder))
            ),
        ),
    })
}

/// The keys of the banner, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum BannerKey {
    Follow,
    Locate,
    Unfavourite,
    Forget,
}

impl State {
    /// A folder's state wherever it shows: inside an opened folder out of reach, that folder's.
    pub(in crate::app) fn folder_presence(&self, path: &str) -> Presence {
        let opened = self.opened_presence();
        if !opened.is_present() && self.opened.as_deref().is_some_and(|o| paths::holds(o, path)) {
            return opened;
        }
        self.row_presence(path)
    }

    /// A tile's marks beyond its flags: its state, its tooltip and its seal's words for a pass gone
    /// or a format no reader of ONI takes.
    pub(in crate::app) fn tile_marks(&self, path: &str) -> (crate::Reach, SharedString, SharedString) {
        let presence = self.presence(path);
        if !presence.is_present() {
            return (reach(presence), tile_note(presence, path, &self.nav.computer).into(), SharedString::new());
        }
        if self.survey.cloud.contains(path) {
            return (crate::Reach::Here, tr("In the cloud: opening it downloads it").into(), SharedString::new());
        }
        if let Some(format) = self.org.seen.unsupported.get(path) {
            let note = tr!("Format not supported ({format})", format = format);
            return (crate::Reach::Here, note.into(), format.into());
        }
        match self.org.seen.gone_passes.get(path) {
            Some((channel, name)) => {
                let (seal, note) = pass_words(*channel, name);
                (crate::Reach::Here, note.into(), seal.into())
            }
            None => (crate::Reach::Here, SharedString::new(), SharedString::new()),
        }
    }

    /// The lead an identity suggests first; on a folder gone, Locate, then what forgets it.
    fn banner_keys(&self) -> Vec<BannerKey> {
        let mut keys = Vec::new();
        if self.lead().is_some() {
            keys.push(BannerKey::Follow);
        }
        if self.opened_presence() == Presence::Missing {
            keys.push(BannerKey::Locate);
            if self.opened.as_deref().is_some_and(|o| self.nav.favourites.contains(o)) {
                keys.push(BannerKey::Unfavourite);
            }
            if !self.rows.is_empty() {
                keys.push(BannerKey::Forget);
            }
        }
        keys
    }

    /// The status bar's word on the originals out of reach among those shown, empty for none.
    pub(in crate::app) fn missing_words(&self) -> Option<String> {
        let n = self.org.missing.len();
        (n > 0 && !self.trash_shown()).then(|| trn("{n} original missing", "{n} originals missing", n as u64))
    }
}

impl Ctl {
    /// The banner of the opened folder out of reach, or none.
    pub(in crate::app) fn publish_banner(&self) {
        let Some(w) = self.window() else { return };
        self.look_for_lead();
        let (shown, keys) = {
            let s = self.state.borrow();
            let folder = s.opened.clone().filter(|_| !s.trash_shown() && !s.org.projects_surface);
            let presence = s.opened_presence();
            let mut words = folder.as_deref().and_then(|f| banner_words(presence, f, &s.nav.computer, !s.order.is_empty()));
            if let (Some((_, message)), Some(lead)) = (words.as_mut(), s.lead()) {
                *message = format!("{message} {}", super::super::navigator::lead_words(lead));
            }
            (words.map(|w| (w, reach(presence))), s.banner_keys())
        };
        let lib = w.global::<Library>();
        lib.set_banner_shown(shown.is_some());
        if let Some(((title, message), reach)) = shown {
            lib.set_banner_title(title.into());
            lib.set_banner_message(message.into());
            lib.set_banner_reach(reach);
        }
        let words: Vec<SharedString> = keys.iter().map(|k| match k {
            BannerKey::Follow => tr("FOLLOW").into(),
            BannerKey::Locate => tr("LOCATE…").into(),
            BannerKey::Unfavourite => tr("REMOVE FROM FAVOURITES").into(),
            BannerKey::Forget => tr("REMOVE FROM LIBRARY…").into(),
        }).collect();
        lib.set_banner_keys(ModelRc::new(VecModel::from(words)));
    }

    pub(in crate::app) fn banner_acted(self: &Rc<Self>, i: usize) {
        let (key, opened) = {
            let s = self.state.borrow();
            (s.banner_keys().get(i).copied(), s.opened.clone())
        };
        let Some(folder) = opened else { return };
        match key {
            Some(BannerKey::Follow) => self.follow_lead(),
            Some(BannerKey::Locate) => self.locate(&folder),
            Some(BannerKey::Unfavourite) => {
                self.toggle_favourite(&folder);
                self.publish_banner();
            }
            Some(BannerKey::Forget) => self.ask_forget(&folder),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_picture_is_grey_and_kept_until_its_thumbnail_is_drawn_again() {
        let seen = Seen::default();
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(2, 1);
        buffer.make_mut_slice()[0] = Rgba8Pixel { r: 200, g: 40, b: 10, a: 255 };
        let colour = Image::from_rgba8(buffer);
        let grey = shown(&seen, "f", colour.clone(), true).to_rgba8().unwrap();
        let p = grey.as_slice()[0];
        assert_eq!((p.r, p.g, p.b, p.a), (72, 72, 72, 255), "its luma, its alpha kept");
        let again = shown(&seen, "f", colour.clone(), true).to_rgba8().unwrap();
        assert_eq!(again.as_bytes().as_ptr(), grey.as_bytes().as_ptr(), "made once");
        // Twins: a present one keeps its colour; a thumbnail drawn again is greyed afresh.
        let kept = shown(&seen, "f", colour.clone(), false).to_rgba8().unwrap();
        assert_eq!(kept.as_slice()[0].r, 200);
        let redrawn = Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::new(2, 1));
        let fresh = shown(&seen, "f", redrawn, true).to_rgba8().unwrap();
        assert_ne!(fresh.as_bytes().as_ptr(), grey.as_bytes().as_ptr());
    }

    #[test]
    fn each_state_has_its_own_words_and_a_folder_there_none() {
        let computer = Computer { home: "/Users/a".into(), volumes: "/Volumes".into() };
        let (caption, help) = row_words(Presence::Offline(Away::Disconnected), "/Volumes/T7/Scans", &computer);
        assert_eq!(caption, "T7 not connected");
        assert!(help.contains("“T7”"), "{help}");
        assert_eq!(row_words(Presence::Missing, "/Users/a/Scans", &computer).0, "Not found");
        assert_eq!(row_words(Presence::Offline(Away::Denied), "/Users/a/Scans", &computer).0, "Access denied");
        assert_eq!(row_words(Presence::Present, "/Users/a/Scans", &computer), (String::new(), String::new()));
        let kept = banner_words(Presence::Offline(Away::Disconnected), "/Volumes/T7/Roll 12", &computer, true).unwrap();
        assert_eq!(kept.0, "T7 Not Connected");
        assert!(kept.1.starts_with("“Roll 12” is on “T7”") && kept.1.contains("last thumbnails"));
        let bare = banner_words(Presence::Offline(Away::Disconnected), "/Volumes/T7/Roll 12", &computer, false).unwrap();
        assert!(!bare.1.contains("thumbnails"), "none kept, none promised: {}", bare.1);
        assert!(banner_words(Presence::Missing, "/Users/a/Scans/Roll 12", &computer, false).unwrap().1.contains("“Scans”"));
        assert_eq!(banner_words(Presence::Present, "/Users/a/Scans", &computer, true), None);
    }
}
