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
    reqwest::Client::builder().timeout(TIMEOUT).user_agent(crate::updates::user_agent()).build().map_err(|e| e.to_string())
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

/// A body read whole, as bytes: the update feed and the archive it names.
pub fn bytes(url: &str) -> Result<Vec<u8>, String> {
    run(async {
        let response = client()?.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
        response.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    })
}

/// A JSON body posted with a 30 s limit, as `BugReport.send` posts it; the status code answered.
pub fn post_json(url: &str, body: String) -> Result<u16, String> {
    run(async {
        let response = client()?
            .post(url)
            .timeout(Duration::from_secs(30))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(response.status().as_u16())
    })
}

/// Where a download from `url` lands: the link's last name made one portable file inside `folder`,
/// as a decoded `%5C` or `C:` would lead a Windows path out of it.
fn landing(url: &str, folder: &Path) -> PathBuf {
    let name = url::Url::parse(url)
        .ok()
        .map(|u| oni_colour::paper_source::decoded_path(&u))
        .and_then(|p| p.rsplit('/').next().map(oni_library::file_names::portable))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "download".into());
    oni_library::file_names::inside(folder, &name).unwrap_or_else(|| folder.join("download"))
}

/// `URLSession.download(from:)`: the file written into `folder` under the link's own name.
pub fn download(url: &str, folder: &Path) -> Result<PathBuf, String> {
    let path = landing(url, folder);
    let bytes = run(async {
        let response = client()?.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
        response.bytes().await.map_err(|e| e.to_string())
    })?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_lands_inside_its_folder_whatever_the_link_spells() {
        let folder = Path::new("/scratch/paper-download-1");
        for url in [
            "https://cdn.shopify.com/x/..%5C..%5C..%5CUsers%5CPublic%5Cfoo.icc.zip",
            "https://cdn.shopify.com/x/C:%5CUsers%5Cfoo.icc",
            "https://cdn.shopify.com/x/..",
        ] {
            let path = landing(url, folder);
            assert_eq!(path.parent(), Some(folder), "{url} → {}", path.display());
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(!name.contains(['\\', ':', '/']) && name != "..", "{url} → {name}");
        }
        // Twin: a plain link keeps its own name.
        assert_eq!(landing("https://www.example.com/profiles/Hahnemuhle%20PhotoRag.icc", folder), folder.join("Hahnemuhle PhotoRag.icc"));
    }
}
