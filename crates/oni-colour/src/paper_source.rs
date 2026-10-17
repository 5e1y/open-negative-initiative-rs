//! Where a paper or a lab publishes its ICC profiles. Links only: nothing is shipped or mirrored.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::profile::{self, OutputProfile};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Reach {
    Direct { hosts: Vec<&'static str>, accepts: &'static str },
    Single(&'static str),
    /// Behind a login, a form or a script: the page opens in the browser.
    Browser,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaperSource {
    pub name: &'static str,
    pub page: &'static str,
    pub reach: Reach,
    pub note: &'static str,
}

fn source(name: &'static str, page: &'static str, reach: Reach, note: &'static str) -> PaperSource {
    PaperSource { name, page, reach, note }
}

impl PaperSource {
    pub fn id(&self) -> &str {
        self.name
    }

    pub fn all() -> Vec<PaperSource> {
        use Reach::*;
        let browse = "Pick the printer and the paper on the page.";
        let archive = "One archive per printer holds every paper.";
        vec![
            source("Awagami", "https://awagami.com/pages/icc-profiles-and-printing-tips",
                   Direct { hosts: vec!["cdn.shopify.com", "awagami.com"], accepts: r"\.icc\.zip$" },
                   "One profile per paper and printer."),
            // Read on `.com`: the page's links are relative and the `.fr` host refuses the files.
            source("Picto (Paris)", "https://www.pictoonline.com/aide/nos-papiers-supports",
                   Direct { hosts: vec!["www.pictoonline.com"], accepts: r"\.icc$" },
                   "The lab's printers, one profile per process and paper."),
            source("WhiteWall", "https://service.whitewall.com/hc/en-us/articles/213813645-Does-WhiteWall-offer-color-management-ICC-color-profiles",
                   Direct { hosts: vec!["static.whitewall.com"], accepts: r"\.ic[cm]$" },
                   "WhiteWall states these are for soft proofing only, not for converting files."),
            source("Saal Digital", "https://www.saal-digital.com/service/professional-zone/softproof-and-icc-profile/",
                   Single("https://www.saal-digital.eu/fileadmin/downloads/softproof/SaalDigital_ICC.zip"),
                   "Every product in one 44 MB archive."),
            source("Hahnemühle", "https://www.hahnemuehle.com/en/digital-papers/icc-profile/download-center.html", Browser, browse),
            source("Canson Infinity", "https://www.canson-infinity.com/en/icc-profiles", Browser, browse),
            source("Ilford Galerie", "https://ilford.com/printer-profiles-paper-settings/", Browser,
                   "The page asks for the printer through a form."),
            source("Moab", "https://www.moabpaper.com/icc-profiles-downloads", Browser, archive),
            source("Red River", "https://www.redrivercatalog.com/profiles/", Browser, archive),
            source("Fotospeed", "https://fotospeed.com/profiles/", Browser, "One profile per paper and printer."),
            source("Tecco", "https://www.tecco.de/icc/", Browser, "Behind a ColorBase account."),
            source("Canon (other makers' papers)", "https://asia.canon/en/support/0200618501", Browser,
                   "Canon allows personal use only, and no redistribution."),
        ]
    }

    // MARK: - Reading a maker's page

    /// The https links of a page on the given hosts whose decoded path the pattern accepts.
    pub fn links(html: &str, page: &str, hosts: &[&str], accepts: &str) -> Vec<Url> {
        let (Ok(href), Ok(accepted), Ok(base)) = (
            RegexBuilder::new(r#"(?:href|src)\s*=\s*["']([^"']+)["']"#).case_insensitive(true).build(),
            RegexBuilder::new(accepts).case_insensitive(true).build(),
            Url::parse(page),
        ) else {
            return vec![];
        };
        let mut seen = HashSet::new();
        href.captures_iter(html)
            .filter_map(|c| {
                let raw = c[1].replace("&amp;", "&");
                let url = base.join(&raw).ok()?;
                if url.scheme() != "https" || !hosts.contains(&url.host_str()?) {
                    return None;
                }
                let path = decoded_path(&url);
                (accepted.is_match(&path) && seen.insert(path)).then_some(url)
            })
            .collect()
    }

    pub fn title(url: &Url) -> String {
        let mut name = last_component(url);
        for ext in [".zip", ".icc", ".icm"] {
            if name.to_lowercase().ends_with(ext) {
                name.truncate(name.len() - ext.len());
            }
        }
        name.replace('_', " ")
    }

    /// The files the source offers, `fetch` returning the page's HTML for a direct one.
    pub fn offered<E>(&self, fetch: impl FnOnce(&str) -> Result<String, E>) -> Result<Vec<Url>, E> {
        Ok(match &self.reach {
            Reach::Single(url) => Url::parse(url).into_iter().collect(),
            Reach::Browser => vec![],
            Reach::Direct { hosts, accepts } => Self::links(&fetch(self.page)?, self.page, hosts, accepts),
        })
    }
}

/// A URL's path percent-decoded, as the record and the pattern read it.
pub fn decoded_path(url: &Url) -> String {
    percent_decode_str(url.path()).decode_utf8_lossy().into_owned()
}

fn last_component(url: &Url) -> String {
    let path = decoded_path(url);
    path.trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_string()
}

// MARK: - What a link installed

/// Files each link laid down, by link path; a preference, since it describes this machine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstallRecord(pub BTreeMap<String, Vec<String>>);

impl InstallRecord {
    pub const KEY: &'static str = "paperInstalls";

    /// Installs a downloaded file and records what it laid down under its link.
    pub fn adopt(&mut self, file: &Path, link: &Url, folder: &Path, scratch: &Path) -> (Vec<OutputProfile>, usize) {
        let result = OutputProfile::importing(&[file.to_path_buf()], folder, scratch);
        if !result.0.is_empty() {
            self.0.insert(decoded_path(link), result.0.iter().map(|p| p.path.clone()).collect());
        }
        result
    }

    /// The files a link put in the folder: its recorded install, else the file its name promises.
    pub fn installed_files(&self, link: &Url, folder: &Path) -> Vec<PathBuf> {
        let recorded: Vec<PathBuf> = self
            .0
            .get(&decoded_path(link))
            .map(|paths| paths.iter().map(PathBuf::from).filter(|p| p.exists()).collect())
            .unwrap_or_default();
        if !recorded.is_empty() {
            return recorded;
        }
        let mut name = last_component(link);
        if name.to_lowercase().ends_with(".zip") {
            name.truncate(name.len() - 4);
        }
        let ext = Path::new(&name).extension().map(|e| e.to_string_lossy().to_lowercase());
        if !matches!(ext.as_deref(), Some("icc" | "icm")) {
            return vec![];
        }
        let guess = folder.join(name);
        if guess.exists() { vec![guess] } else { vec![] }
    }

    /// Discards what a link installed, sending it where `discard` says, and forgets the link.
    pub fn uninstall<E>(&mut self, link: &Url, folder: &Path, mut discard: impl FnMut(&Path) -> Result<(), E>) -> Vec<String> {
        let removed = self
            .installed_files(link, folder)
            .iter()
            .filter(|f| discard(f).is_ok())
            .map(|f| profile::standardized(f))
            .collect();
        self.0.remove(&decoded_path(link));
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_yields_its_profiles_and_nothing_else() {
        let html = r#"<a href="/files/Baryta_Gloss.icc">x</a> <a HREF='https://www.pictoonline.com/files/Baryta_Gloss.icc'>dup</a>
            <a href="/files/notice.pdf">pdf</a> <a href="http://www.pictoonline.com/files/Plain.icc">http</a>
            <img src="https://evil.example/x.icc"> <a href="/files/Mat%20Fine.ICC?a=1&amp;b=2">mat</a>"#;
        let links = PaperSource::links(html, "https://www.pictoonline.com/aide/nos-papiers-supports", &["www.pictoonline.com"], r"\.icc$");
        let paths: Vec<String> = links.iter().map(decoded_path).collect();
        assert_eq!(paths, vec!["/files/Baryta_Gloss.icc", "/files/Mat Fine.ICC"]);
        assert_eq!(links[1].query(), Some("a=1&b=2"));
        assert_eq!(PaperSource::title(&links[0]), "Baryta Gloss");
        // Twin: another host's list refuses every link of the page.
        assert!(PaperSource::links(html, "https://www.pictoonline.com/", &["static.whitewall.com"], r"\.icc$").is_empty());
    }

    #[test]
    fn a_maker_s_page_yields_each_profile_once_relative_ones_resolved() {
        let html = r#"<a href="//cdn.shopify.com/s/files/1/2472/3474/files/Awagami_Kozo_P900.icc.zip?v=17">Kozo</a>
            <a href="https://cdn.shopify.com/s/files/1/2472/3474/files/Awagami_Kozo_P900.am1x.zip">preset</a>
            <a href="https://evil.example.com/files/Awagami_Bamboo.icc.zip">elsewhere</a>
            <a href="/files/Awagami_Bamboo_Canon.icc.zip">relative</a>"#;
        let page = "https://awagami.com/pages/icc-profiles-and-printing-tips";
        let found = PaperSource::links(html, page, &["cdn.shopify.com", "awagami.com"], r"\.icc\.zip$");
        let names: Vec<String> = found.iter().map(last_component).collect();
        assert_eq!(names, vec!["Awagami_Kozo_P900.icc.zip", "Awagami_Bamboo_Canon.icc.zip"]);
        // Twin: without the host and the pattern the preset and the foreign host come in too.
        let loose = PaperSource::links(html, page, &["cdn.shopify.com", "awagami.com", "evil.example.com"], r"\.zip$");
        assert_eq!(loose.len(), 4);
    }

    #[test]
    fn a_title_drops_every_extension_in_turn() {
        let url = Url::parse("https://cdn.shopify.com/x/Awagami_Bamboo_Canon.icc.zip").unwrap();
        assert_eq!(PaperSource::title(&url), "Awagami Bamboo Canon");
        // Twin: a name with no known extension keeps its dot.
        let other = Url::parse("https://cdn.shopify.com/x/Read_me.pdf").unwrap();
        assert_eq!(PaperSource::title(&other), "Read me.pdf");
    }

    #[test]
    fn the_sources_are_links_and_a_single_offers_itself() {
        let all = PaperSource::all();
        assert_eq!(all.len(), 12);
        assert!(all.iter().all(|s| Url::parse(s.page).is_ok_and(|u| u.scheme() == "https")));
        let saal = all.iter().find(|s| s.name == "Saal Digital").unwrap();
        let offered = saal.offered(|_| Err::<String, ()>(())).unwrap();
        assert_eq!(offered.len(), 1);
        // Twin: a direct source asks for its page, and a failing fetch is reported.
        assert!(all[0].offered(|_| Err::<String, ()>(())).is_err());
    }
}
