//! The simulated The Lens DB on its own, to look at its pages in a browser: `--port`, `--catalogue <file>`
//! and `--kept <file>` as the app's `ONI_TLDB=fake` would set them.

use oni_tldb::fake::{self, Setup};
use std::path::PathBuf;

fn main() {
    let mut setup = Setup::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next().unwrap_or_else(|| panic!("{arg} wants a value"));
        match arg.as_str() {
            "--port" => setup.port = value.parse().expect("a port number"),
            "--catalogue" => setup.catalogue = Some(fake::read_catalogue(&PathBuf::from(value)).expect("a readable catalogue")),
            "--kept" => setup.kept = Some(PathBuf::from(value)),
            other => panic!("unknown option {other}"),
        }
    }
    let fake = fake::start_with(setup).expect("a loopback port");
    println!("{}", fake.site);
    loop {
        std::thread::park();
    }
}
