// The Windows executable carries the app's icon, which the Explorer, the taskbar and the installer show,
// and its manifest. winit sets per-monitor DPI awareness (v2) itself when its event loop starts.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/windows/oni.ico");
    stock_artwork();
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../packaging/windows/oni.ico");
        resource.set_manifest(MANIFEST);
        resource.compile().expect("the Windows resources compile");
    }
}

/// Every canister in the stocks' folder, keyed by its file's name: a stock's artwork arrives by dropping its SVG there.
fn stock_artwork() {
    const FOLDER: &str = "../oni-ui/ui/stocks";
    println!("cargo:rerun-if-changed={FOLDER}");
    let mut names: Vec<String> = std::fs::read_dir(FOLDER)
        .expect("the stocks' folder reads")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".svg") && !n.starts_with('.'))
        .collect();
    names.sort();
    let rows: String = names
        .iter()
        .map(|n| format!("    ({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{FOLDER}/{n}\"))),\n", n.trim_end_matches(".svg")))
        .collect();
    let table = format!("const ARTWORK: [(&str, &[u8]); {}] = [\n{rows}];\n", names.len());
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("stock_artwork.rs");
    std::fs::write(out, table).expect("the artwork table writes");
}

/// Paths past 260 characters for the Win32 calls std does not prefix (watch, Recycle Bin, Explorer),
/// Common Controls 6 for rfd's dialogs, and no elevation asked.
#[cfg(windows)]
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings xmlns:ws2="http://schemas.microsoft.com/SMI/2016/WindowsSettings">
      <ws2:longPathAware>true</ws2:longPathAware>
    </windowsSettings>
  </application>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;
