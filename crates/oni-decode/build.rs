fn main() {
    let lib = pkg_config::probe_library("libraw").expect("LibRaw not found by pkg-config");
    let mut build = cc::Build::new();
    build.file("csrc/pinned.c");
    for path in &lib.include_paths {
        build.include(path);
    }
    build.compile("oni_pinned");
    println!("cargo:rerun-if-changed=csrc/pinned.c");
}
