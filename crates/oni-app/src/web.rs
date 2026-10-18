//! The app's reach to the web: a maker's page read as text and a file downloaded, over HTTPS checked
//! against the system's trust store. Checks inject their own and never reach the network.

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;

/// Long enough for a 44 MB archive on a slow line, short enough that a dead host gives up.
const TIMEOUT: Duration = Duration::from_secs(300);

fn client() -> Result<reqwest::Client, String> {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    reqwest::Client::builder().timeout(TIMEOUT).build().map_err(|e| e.to_string())
}

fn run<T>(work: impl std::future::Future<Output = Result<T, String>>) -> Result<T, String> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?.block_on(work)
}

/// `URLSession.data(from:)`: the page's body as text, on the calling thread.
pub fn text(url: &str) -> Result<String, String> {
    run(async {
        let response = client()?.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
        response.text().await.map_err(|e| e.to_string())
    })
}

/// `URLSession.download(from:)`: the file written into `folder` under the link's own name.
pub fn download(url: &str, folder: &Path) -> Result<PathBuf, String> {
    let name = url::Url::parse(url)
        .ok()
        .map(|u| oni_colour::paper_source::decoded_path(&u))
        .and_then(|p| p.rsplit('/').next().map(str::to_owned))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "download".into());
    let bytes = run(async {
        let response = client()?.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
        response.bytes().await.map_err(|e| e.to_string())
    })?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = folder.join(name);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}
