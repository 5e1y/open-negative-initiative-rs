//! The welcome's De-dust scene's pictures from a dusty scan: the detail the canvas shows, as it is and
//! healed by the app's own patches, each speck the reading finds in it at rest, and each patch's tile.
use oni_demos::welcome::dedust::{Scan, View, found, framing, within};
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("usage: dedust_assets SCAN OUT WIDTH HEIGHT ZOOM MOST")?;
    let out = args.next().ok_or("no OUT")?;
    let width: u32 = args.next().ok_or("no WIDTH")?.parse().map_err(|_| "WIDTH")?;
    let height: u32 = args.next().ok_or("no HEIGHT")?.parse().map_err(|_| "HEIGHT")?;
    let zoom: f32 = args.next().ok_or("no ZOOM")?.parse().map_err(|_| "ZOOM")?;
    let most: usize = args.next().ok_or("no MOST")?.parse().map_err(|_| "MOST")?;
    let out = Path::new(&out);
    let scan = Scan::open(Path::new(&source))?;
    let (reading, _, forms) = scan.read()?;
    let all = found(&reading, 0.0);
    let ring = 14.0;
    let (view, n) = framing(&all, scan.size, (width, height), zoom, ring, most, &forms).ok_or("no detail holds its specks apart")?;
    let shown = within(&all, view, scan.size, ring);
    eprintln!("{} specks in the frame, {n} in the detail at {}×, from {} {}", all.len(), view.zoom, view.x, view.y);
    scan.detail(&scan.settings, view)?.png(&out.join("dusty.png"))?;
    let mut healed = scan.settings.clone();
    healed.corrections = scan.corrections(&shown);
    scan.heal(&healed, |_, _| Ok(()))?;
    scan.detail(&healed, view)?.png(&out.join("healed.png"))?;
    for (i, tile) in scan.originals(&scan.settings, &shown)?.iter().enumerate() {
        tile.png(&out.join(format!("tile-{}.png", i + 1)))?;
    }
    // Each speck's centre and its ring's diameter, in the detail's pixels.
    let mut json = std::fs::File::create(out.join("specks.json")).map_err(|e| e.to_string())?;
    let lines: Vec<String> = shown
        .iter()
        .map(|f| {
            let [x0, y0, x1, y1] = f.bounds.map(f64::from);
            let cx = ((x0 + x1) / 2.0 - f64::from(view.x)) * f64::from(view.zoom);
            let cy = ((y0 + y1) / 2.0 - f64::from(view.y)) * f64::from(view.zoom);
            let d = (x1 - x0).max(y1 - y0) * f64::from(view.zoom);
            format!("{{\"x\": {cx:.1}, \"y\": {cy:.1}, \"d\": {d:.1}}}")
        })
        .collect();
    writeln!(json, "[{}]", lines.join(", ")).map_err(|e| e.to_string())?;
    let _ = View { ..view };
    Ok(())
}
