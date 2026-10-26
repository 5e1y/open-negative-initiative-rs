//! An archive laid again from the archive held: kept, its medals set or its groups cut again when
//! that is all that moved, so a lay on the rows already laid cuts no tree.

use super::*;

/// What an archive was cut from, so a lay on the same rows and survey cuts nothing again.
pub(super) struct Basis {
    pub(super) folder: String,
    pub(super) rows: Vec<Entry>,
    pub(super) survey: Arc<Survey>,
    pub(super) filter: Option<Medal>,
}

impl Basis {
    /// The survey's parts a tree is cut from, the same allocation or equal ones; of the sidecars,
    /// which JPEGs have one, all a pair reads, so an edit's sidecar written moves no tree.
    fn same_survey(&self, survey: &Arc<Survey>) -> bool {
        let (a, b) = (&*self.survey, &**survey);
        let paired = |s: &Survey| -> HashSet<String> {
            let jpeg = |p: &String| oni_library::library::JPEG_EXTENSIONS.iter().any(|k| paths::extension(p).eq_ignore_ascii_case(k));
            s.sidecars.keys().filter(|p| jpeg(p)).cloned().collect()
        };
        Arc::ptr_eq(&self.survey, survey)
            || (a.images == b.images && a.rolls == b.rolls && a.folders == b.folders && a.made == b.made && paired(a) == paired(b))
    }
}

/// What a lay did with the archive it was handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relaid {
    /// Nothing it was cut from moved.
    Kept,
    /// Only medals moved: set on its photos and rows, nothing cut again.
    Medals,
    /// The gallery's groups cut again under a filter, the tree kept.
    Regrouped,
    /// Cut whole from the rows and the survey.
    Cut,
}

static TREES: AtomicUsize = AtomicUsize::new(0);
pub(super) static ROW_LISTS: AtomicUsize = AtomicUsize::new(0);

/// The trees a lay cut or took from a read, and the row lists laid flat, since launch: what a
/// check counts to see that a gesture rebuilt nothing.
pub fn cuts() -> (usize, usize) {
    (TREES.load(Ordering::Relaxed), ROW_LISTS.load(Ordering::Relaxed))
}

/// Two rows alike but for their medal; every field is named, so a field added is never left out.
fn alike(a: &Entry, b: &Entry) -> bool {
    let Entry { path, imported_at, captured_at, fingerprint, label: _, film, manual_order, mode, pass_of, leads_passes, trashed_at } = a;
    *path == b.path
        && *imported_at == b.imported_at
        && *captured_at == b.captured_at
        && *fingerprint == b.fingerprint
        && *film == b.film
        && *manual_order == b.manual_order
        && *mode == b.mode
        && *pass_of == b.pass_of
        && *leads_passes == b.leads_passes
        && *trashed_at == b.trashed_at
}

impl Archive {
    /// Whether this archive was cut from exactly these rows and survey, in `folder`.
    fn cut_from(&self, folder: &str, entries: &[Entry], survey: &Arc<Survey>) -> bool {
        self.basis.folder == folder && self.basis.same_survey(survey) && self.basis.rows == entries
    }

    /// The archive for `entries` and `survey` under `filter`, made from `old` where it can: kept when
    /// nothing it was cut from moved, its medals set when only they did, its groups cut again for a
    /// filter; else `staged`, one cut off this thread from these very rows; else cut here, whole.
    pub fn relaid(old: Option<Archive>, staged: Option<Archive>, folder: &str, entries: &[Entry], survey: &Arc<Survey>, filter: Option<Medal>) -> (Archive, Relaid) {
        let old = old.filter(|a| {
            let b = &a.basis;
            b.folder == folder && b.rows.len() == entries.len() && b.same_survey(survey) && b.rows.iter().zip(entries).all(|(a, e)| alike(a, e))
        });
        let Some(mut archive) = old else {
            TREES.fetch_add(1, Ordering::Relaxed);
            let mut archive = staged.filter(|a| a.cut_from(folder, entries, survey)).unwrap_or_else(|| Archive::from_rows(folder, entries, survey, filter));
            archive.basis.survey = survey.clone();
            if archive.basis.filter != filter {
                archive.regroup(filter);
            }
            return (archive, Relaid::Cut);
        };
        let medals: HashMap<String, Medal> = archive.basis.rows.iter().zip(entries).filter(|(a, e)| a.label != e.label).map(|(_, e)| (e.path.clone(), e.label)).collect();
        archive.basis.survey = survey.clone();
        archive.set_labels(&medals);
        let regroup = archive.basis.filter != filter || (filter.is_some() && !medals.is_empty());
        if regroup {
            archive.regroup(filter);
            return (archive, Relaid::Regrouped);
        }
        (archive, if medals.is_empty() { Relaid::Kept } else { Relaid::Medals })
    }

    /// The gallery's groups cut again under `filter`, the tree as it is.
    fn regroup(&mut self, filter: Option<Medal>) {
        self.basis.filter = filter;
        let survey = &self.basis.survey;
        let (paired, _) = folders::paired(&self.basis.rows, &survey.images, &survey.sidecars);
        self.groups = self.cut_groups(&paired);
    }

    /// Records a label already written to the sidecar, so the tree and the tiles show it.
    pub fn set_label(&mut self, path: &str, label: Medal) {
        self.set_labels(&HashMap::from([(path.to_owned(), label)]));
    }

    /// `set_label` for every path in `labels`, in one walk of the tree, the groups and the rows.
    fn set_labels(&mut self, labels: &HashMap<String, Medal>) {
        if labels.is_empty() {
            return;
        }
        let set = |e: &mut Entry| {
            if let Some(label) = labels.get(&e.path) {
                e.label = *label;
            }
        };
        fn walk(nodes: &mut [Node], set: &dyn Fn(&mut Entry)) {
            for n in nodes {
                match n {
                    Node::Photo(e) => set(e),
                    Node::Folder { group, children, .. } => {
                        walk(children, set);
                        group.iter_mut().flat_map(|g| g.entries.iter_mut()).for_each(set);
                    }
                }
            }
        }
        walk(&mut self.nodes, &set);
        self.groups.iter_mut().flat_map(|g| g.entries.iter_mut()).for_each(set);
        self.basis.rows.iter_mut().for_each(set);
        if let Some(rows) = self.laid.get_mut() {
            for row in rows {
                if let NodeId::Photo(path) = &row.id
                    && let Some(label) = labels.get(path)
                {
                    row.caption = medal(*label);
                }
            }
        }
    }
}
