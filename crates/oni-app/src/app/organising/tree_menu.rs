//! The trees' right click (`filmContextMenu`, `projectContextMenu`): on the tree's pick when it
//! lands inside it, else on its row; photos alone take the cell menu with Edit first when one.

use super::*;
use oni_library::archive_tree::NodeId as ArchiveNode;

impl Ctl {
    /// `filmContextMenu` (`EditorScene+Gallery.swift:112`): a folder alone takes the navigator's
    /// folder menu; photos alone take the cell menu; the rest shows in the Finder.
    pub(in crate::app) fn row_menu(self: &Rc<Self>, i: usize) -> bool {
        let can_paste = self.can_paste();
        let (items, path) = {
            let s = self.state.borrow();
            let Some(id) = s.org.tree_ids.get(i).cloned() else { return false };
            let targets = s.org.tree_pick.targets(&id, &s.org.tree_ids);
            let photos: Vec<String> = targets.iter().filter_map(|t| if let ArchiveNode::Photo(p) = t { Some(p.clone()) } else { None }).collect();
            if let [ArchiveNode::Folder(folder)] = targets.as_slice() {
                let lines = s.folder_lines(folder, s.folder_presence(folder));
                (lines.into_iter().map(|(line, act)| Item { line, act: Act::Folder(act, folder.clone()) }).collect(), None)
            } else if !photos.is_empty() && photos.len() == targets.len() {
                let scope: HashSet<String> = photos.iter().cloned().collect();
                (self.cell_items(&s, &photos[0], &scope, true, can_paste), Some(photos[0].clone()))
            } else {
                let paths: Vec<String> = targets
                    .iter()
                    .filter_map(|t| match t {
                        ArchiveNode::Photo(p) | ArchiveNode::Folder(p) => Some(p.clone()),
                        ArchiveNode::Roll(_) | ArchiveNode::Day(_) => None,
                    })
                    .collect();
                (vec![Item { line: menu_line(crate::app::navigator::reveal_word(), true, false, false), act: Act::Reveal(paths) }], None)
            }
        };
        self.open_menu(items, path)
    }

    /// `projectContextMenu` (`EditorScene+Rolls.swift:179`): photos alone take the cell menu; a
    /// project alone, Delete Project; several projects open nothing.
    pub(in crate::app) fn project_row_menu(self: &Rc<Self>, i: usize) -> bool {
        let can_paste = self.can_paste();
        let (items, path) = {
            let s = self.state.borrow();
            let Some(id) = s.org.project_nodes.get(i).cloned() else { return false };
            let targets = s.org.project_pick.targets(&id, &s.org.project_nodes);
            let projects: Vec<Id> = targets.iter().filter_map(|t| if let NodeId::Project(p) = t { Some(*p) } else { None }).collect();
            match projects.as_slice() {
                [] => {
                    // A photo shown under two projects is one photo.
                    let mut photos: Vec<String> = Vec::new();
                    for t in &targets {
                        if let NodeId::Photo { path, .. } = t
                            && !photos.contains(path)
                        {
                            photos.push(path.clone());
                        }
                    }
                    let Some(first) = photos.first().cloned() else { return false };
                    let scope: HashSet<String> = photos.into_iter().collect();
                    (self.cell_items(&s, &first, &scope, true, can_paste), Some(first))
                }
                [project] => (vec![Item { line: menu_line(tr("Delete Project"), true, false, false), act: Act::DeleteProject(*project) }], None),
                _ => return false,
            }
        };
        self.open_menu(items, path)
    }
}
