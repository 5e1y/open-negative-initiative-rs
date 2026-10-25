// The Windows executable carries the app's icon, which the Explorer, the taskbar and the installer show.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/windows/oni.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../packaging/windows/oni.ico");
        resource.compile().expect("the Windows resources compile");
    }
}
