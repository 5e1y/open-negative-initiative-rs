//! The destinations of an export and the dialog a clash opens: Replace for ONI's own earlier
//! exports alone, Rename the Finder's way as soon as an original or a file of another app is met.

use oni_export::landing::Plan;
use oni_export::{Format, Row};
use oni_i18n::{tr, trn};
use std::path::{Path, PathBuf};

use super::Target;

/// The destinations of a batch in a folder: each file the name beside its preview, inside the folder.
pub fn destinations(folder: &Path, targets: &[Target], rows: &[Option<Row>], format: Format) -> Vec<PathBuf> {
    targets.iter().zip(rows).map(|(t, r)| oni_export::destination(folder, &t.path, r.as_ref().map_or("", |r| r.name.as_str()), format)).collect()
}

/// What the dialog says, its one key, and the files that key writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub title: String,
    pub message: String,
    /// Replace when every clash is an earlier export, else Rename.
    pub replaces: bool,
    pub destinations: Vec<PathBuf>,
}

/// The clash dialog of a plan, `None` when nothing stands in the way.
pub fn notice(plan: &Plan) -> Option<Notice> {
    let (kept, exports, of) = (plan.kept(), plan.exports(), plan.named.len());
    if kept > 0 {
        let renamed = plan.renamed();
        let example = plan.clashes.iter().zip(&renamed).find(|(c, _)| **c != oni_export::landing::Clash::None);
        let example = example.and_then(|(_, p)| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let n = (kept + exports) as u64;
        return Some(Notice {
            title: trn("{n} Name Is Already Taken in That Folder", "{n} Names Are Already Taken in That Folder", n),
            message: tr!(
                "Each file whose name is taken gets a number instead, such as “{example}”. Nothing in the folder is replaced, and the originals are not touched.",
                example = example
            ),
            replaces: false,
            destinations: renamed,
        });
    }
    if exports == 0 {
        return None;
    }
    let title = trn("{n} File Already Exists in That Folder", "{n} Files Already Exist in That Folder", exports as u64);
    let message = if exports == of {
        tr("Exporting will replace them. The originals are not touched.").to_owned()
    } else {
        tr!("Exporting will replace {clashes} of the {of} files. The originals are not touched.", clashes = exports, of = of)
    };
    Some(Notice { title, message, replaces: true, destinations: plan.named.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_export::landing::Clash;

    #[test]
    fn a_clash_offers_replace_for_earlier_exports_alone() {
        let p = |n: &str| PathBuf::from(format!("/nowhere-oni/{n}"));
        let exports = Plan { named: vec![p("a.jpg"), p("b.jpg"), p("c.jpg")], clashes: vec![Clash::Export, Clash::None, Clash::None] };
        let asked = notice(&exports).unwrap();
        assert!(asked.replaces);
        assert_eq!(asked.title, "1 File Already Exists in That Folder");
        assert_eq!(asked.message, "Exporting will replace 1 of the 3 files. The originals are not touched.");
        assert_eq!(asked.destinations, exports.named);
        let all = Plan { named: vec![p("a.jpg")], clashes: vec![Clash::Export] };
        assert_eq!(notice(&all).unwrap().message, "Exporting will replace them. The originals are not touched.");
        // An original among them turns every clash into a new name, and Replace is never offered.
        let mixed = Plan { named: vec![p("a.jpg"), p("b.jpg"), p("c.jpg")], clashes: vec![Clash::Export, Clash::Original, Clash::None] };
        let asked = notice(&mixed).unwrap();
        assert!(!asked.replaces);
        assert_eq!(asked.title, "2 Names Are Already Taken in That Folder");
        assert_eq!(asked.destinations, [p("a 2.jpg"), p("b 2.jpg"), p("c.jpg")]);
        assert!(asked.message.contains("“a 2.jpg”"), "{}", asked.message);
        // Twin: a folder where nothing stands asks nothing.
        assert_eq!(notice(&Plan { named: vec![p("a.jpg")], clashes: vec![Clash::None] }), None);
    }
}
