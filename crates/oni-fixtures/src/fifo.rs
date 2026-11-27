//! A read held on a FIFO standing in for a file, so a check acts while the read waits inside it.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

/// Makes `path` a FIFO, which a read of it then waits on until `give` writes it.
pub fn make(path: &Path) {
    assert!(std::process::Command::new("mkfifo").arg(path).status().unwrap().success(), "mkfifo {}", path.display());
}

/// Once a reader opens the FIFO at `path`, runs `meanwhile`, writes `bytes` and closes it; then, until
/// `ended` says the read is over, hands its reader the end of the file.
pub fn give(path: &Path, bytes: &[u8], meanwhile: impl FnOnce(), ended: impl Fn() -> bool) {
    // Opening to write waits for the reader to open: it is past any check before its read.
    let mut pipe = std::fs::File::options().write(true).open(path).unwrap();
    meanwhile();
    pipe.write_all(bytes).unwrap();
    drop(pipe);
    // macOS can leave a FIFO's reader waiting after its last writer closed (5 in 3000 runs): each
    // further open and close for writing, read too so it never blocks, hands it the end of the file.
    while !ended() {
        drop(std::fs::File::options().read(true).write(true).open(path));
        std::thread::sleep(Duration::from_millis(20));
    }
}
