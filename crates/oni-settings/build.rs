// Every file of the stocks' folder built into the binary, by its name: a stock or a paper ships by dropping its file there.
fn main() {
    const FOLDER: &str = "stocks";
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={FOLDER}");
    let mut names: Vec<String> = std::fs::read_dir(FOLDER)
        .expect("the stocks' folder reads")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| (n.ends_with(".json") || n.ends_with(".svg")) && !n.starts_with('.'))
        .collect();
    names.sort();
    let rows: String =
        names.iter().map(|n| format!("    ({n:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{FOLDER}/{n}\"))),\n")).collect();
    let table = format!("pub const SHIPPED: [(&str, &[u8]); {}] = [\n{rows}];\n", names.len());
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("shipped_stocks.rs");
    std::fs::write(out, table).expect("the stocks' table writes");
}
