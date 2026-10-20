//! `restore <support folder> [folder]`: every sidecar the transition corrected, or those under the
//! folder, back to the bytes it was found with; one written since its correction is kept and named.

fn main() {
    let support = std::env::args().nth(1).expect("the app's support folder");
    let folder = std::env::args().nth(2).map(std::path::PathBuf::from);
    let done = oni_transition::correct::restore(std::path::Path::new(&support), folder.as_deref());
    for path in &done.restored {
        println!("restored {path}");
    }
    for (path, why) in &done.kept {
        println!("kept {path}: {why}");
    }
    println!("{} restored, {} kept", done.restored.len(), done.kept.len());
}
