//! Stills of a welcome scene, named by `SCENE`, at the moments named on the command line, in seconds.
use std::path::Path;
use std::time::Duration;

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).ok_or("usage: clarity_stills OUT T...")?;
    let name = std::env::var("SCENE").unwrap_or_else(|_| "clarity".into());
    let scene = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("scenes/stills/{name}.slint"));
    let component = match name.as_str() { "clarity" => "Clarity", "favourites" => "Favourites", "edit" => "Edit", "dust" => "Dust", "fixes" => "Fixes", _ => "Focus" };
    let stage = oni_demos::stage::Stage::new(&scene, component, (560.0, 502.0))?;
    for t in std::env::args().skip(2) {
        let s: f64 = t.parse().map_err(|_| format!("not a time: {t}"))?;
        stage.shoot(Duration::from_secs_f64(s))?.png(&Path::new(&out).join(format!("t{t}.png")))?;
    }
    Ok(())
}
