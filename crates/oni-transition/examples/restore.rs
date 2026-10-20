//! `restore <support folder>`: every sidecar the transition corrected back to the bytes it was found
//! with, from `<support>/Transition/backup/`; one written since its correction is kept and named.

fn main() {
    let support = std::env::args().nth(1).expect("the app's support folder");
    let done = oni_transition::correct::restore(std::path::Path::new(&support));
    for path in &done.restored {
        println!("restored {path}");
    }
    for (path, why) in &done.kept {
        println!("kept {path}: {why}");
    }
    println!("{} restored, {} kept", done.restored.len(), done.kept.len());
}
