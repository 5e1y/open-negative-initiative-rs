//! The trees' right click (`filmContextMenu`, `projectContextMenu`): on the tree's pick when it
//! lands inside it, else on its row; photos alone take the cell menu with Edit first when one.

use super::*;
use oni_library::archive_tree::NodeId as ArchiveNode;

impl Ctl {
    /// `filmContextMenu` (`EditorScene+Gallery.swift:112`): a folder alone is renamed, made a roll
    /// or given a folder inside; photos alone take the cell menu; the rest shows in the Finder.
    pub(in crate::app) fn row_menu(self: &Rc<Self>, i: usize) -> bool {
        let can_paste = self.can_paste();
        let (items, path) = {
            let s = self.state.borrow();
            let Some(id) = s.org.tree_ids.get(i).cloned() else { return false };
            let targets = s.org.tree_pick.targets(&id, &s.org.tree_ids);
            let mut items = Vec::new();
            if let [ArchiveNode::Folder(folder)] = targets.as_slice() {
                let roll = s.nav.rolls().contains(folder);
                // Out of reach, a folder takes nothing that writes in it.
                let here = s.folder_presence(folder).is_present();
                items.push(Item { line: menu_line(tr("Rename…"), here, false, false), act: Act::RenameFolder(folder.clone()) });
                let text = if roll { tr("Back to a Folder") } else { tr("Make a Roll") };
                items.push(Item { line: menu_line(text, here, false, false), act: Act::Roll(folder.clone(), !roll) });
                items.push(Item { line: menu_line(tr("New Folder Inside"), here, false, false), act: Act::NewFolder(folder.clone()) });
            }
            let photos: Vec<String> = targets.iter().filter_map(|t| if let ArchiveNode::Photo(p) = t { Some(p.clone()) } else { None }).collect();
            if !photos.is_empty() && photos.len() == targets.len() {
                let scope: HashSet<String> = photos.iter().cloned().collect();
                items.extend(self.cell_items(&s, &photos[0], &scope, true, can_paste));
                (items, Some(photos[0].clone()))
            } else {
                let paths: Vec<String> = targets
                    .iter()
                    .filter_map(|t| match t {
                        ArchiveNode::Photo(p) | ArchiveNode::Folder(p) => Some(p.clone()),
                        ArchiveNode::Roll(_) | ArchiveNode::Day(_) => None,
                    })
                    .collect();
                let rule = !items.is_empty();
                items.push(Item { line: menu_line(tr("Show in Finder"), true, rule, false), act: Act::Reveal(paths) });
                (items, None)
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
