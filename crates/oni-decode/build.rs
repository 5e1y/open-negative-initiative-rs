//! `ONI_DECODE_STAMP`, the decoder's code as one print (`src/stamp.rs`); LibRaw is built by `oni-libraw`,
//! so an edit here reprints in milliseconds and never compiles LibRaw again.

#[path = "src/stamp.rs"]
mod stamp;

fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo names the crate's folder"));
    for part in stamp::PARTS {
        println!("cargo:rerun-if-changed={part}");
    }
    let print = stamp::stamp(&root, &stamp::PARTS).expect("every part of the decoder's code is readable");
    println!("cargo:rustc-env=ONI_DECODE_STAMP={print}");
}
