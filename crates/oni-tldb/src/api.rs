//! The partner API's calls: the catalogue searched, the Kit read and changed in one call, a token revoked.

use crate::piece::{Key, Kind, Piece};
use serde::{Deserialize, Serialize};
use std::sync::Once;
use std::time::Duration;

/// Where the service is and how ONI names itself to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    /// `https://thelensdb.com`, or the simulated service's local address.
    pub site: String,
    /// `ONI/<version> (<system>)`, which the service's firewall lets through.
    pub agent: String,
}

/// Why a call gave nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// 401: the token is gone; ONI turns signed out and keeps its copy.
    SignedOut,
    /// 429: try again after so many seconds.
    Wait(u64),
    /// 400 or 403: a fault on ONI's side, never retried.
    Refused(String),
    /// 5xx or no answer: the copy stands until the next launch.
    Down(String),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub display_name: String,
}

/// The whole Kit, the oldest piece first; `not_found` names what a change did not know.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kit {
    #[serde(default)]
    pub account: Account,
    #[serde(default)]
    pub items: Vec<Piece>,
    #[serde(default)]
    pub not_found: Vec<Key>,
}

/// A Kit read: unchanged since the ETag sent, or fresh with its own.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    Same,
    Fresh { kit: Kit, etag: Option<String> },
}

/// The calls a signed-in person's token makes.
#[derive(Debug, Clone, PartialEq)]
pub struct Client {
    pub service: Service,
    pub token: String,
}

/// A call that hears nothing for this long counts the service down.
const TIMEOUT: Duration = Duration::from_secs(30);

/// The partner API's version, part of every address.
pub(crate) const BASE: &str = "/api/partner/v1";

impl Service {
    /// The real service, named with this build's version and system.
    pub fn new(site: &str, version: &str) -> Service {
        let system = match std::env::consts::OS {
            "macos" => "macOS",
            "windows" => "Windows",
            other => other,
        };
        Service { site: site.trim_end_matches('/').to_string(), agent: format!("ONI/{version} ({system})") }
    }

    /// A page of the service opened from ONI, carrying `utm_source=oni`.
    pub fn page(&self, url: &str) -> String {
        let Ok(mut parsed) = url::Url::parse(url) else { return url.to_string() };
        if !parsed.query_pairs().any(|(name, _)| name == "utm_source") {
            parsed.query_pairs_mut().append_pair("utm_source", "oni");
        }
        parsed.into()
    }

    /// Where a piece missing from the catalogue is proposed.
    pub fn submit_page(&self) -> String {
        self.page(&format!("{}/submit", self.site))
    }

    /// The token ended at sign-out; the service answers 200 whatever it was.
    pub fn revoke(&self, token: &str) -> Result<(), Failure> {
        let form = url::form_urlencoded::Serializer::new(String::new()).append_pair("token", token).finish();
        let call = Call::post(&format!("{}/api/oauth/revoke", self.site), FORM, form);
        call.send(self, None).map(|_| ())
    }

    /// `POST /api/oauth/token`: the sign-in's code and verifier exchanged for the token.
    pub(crate) fn exchange(&self, code: &str, redirect: &str, verifier: &str) -> Result<String, Failure> {
        #[derive(Deserialize)]
        struct Answer {
            access_token: String,
        }
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("redirect_uri", redirect)
            .append_pair("client_id", crate::CLIENT_ID)
            .append_pair("code_verifier", verifier)
            .finish();
        let answer = Call::post(&format!("{}/api/oauth/token", self.site), FORM, form).send(self, None)?;
        Ok(parse::<Answer>(&answer.body)?.access_token)
    }

    /// A site that is no address is ONI's fault, never retried.
    fn api(&self, path: &str) -> Result<url::Url, Failure> {
        url::Url::parse(&format!("{}{BASE}{path}", self.site)).map_err(|e| Failure::Refused(e.to_string()))
    }
}

impl Client {
    /// `GET /search`: 2 to 200 characters, 10 words at most; `limit` 50 at most.
    pub fn search(&self, query: &str, kind: Option<Kind>, limit: u32) -> Result<Vec<Piece>, Failure> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            results: Vec<Piece>,
        }
        let mut url = self.service.api("/search")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query);
            if let Some(kind) = kind {
                pairs.append_pair("type", kind_name(kind));
            }
            pairs.append_pair("limit", &limit.to_string());
        }
        let answer = Call::get(url.as_str()).send(&self.service, Some(&self.token))?;
        Ok(parse::<Answer>(&answer.body)?.results)
    }

    /// `GET /kit`, with `If-None-Match` when a copy's ETag is known.
    pub fn kit(&self, etag: Option<&str>) -> Result<Read, Failure> {
        let mut call = Call::get(self.service.api("/kit")?.as_str());
        call.etag = etag.map(str::to_string);
        let answer = call.send(&self.service, Some(&self.token))?;
        if answer.status == 304 {
            return Ok(Read::Same);
        }
        Ok(Read::Fresh { kit: parse(&answer.body)?, etag: answer.etag })
    }

    /// `POST /kit/changes`: every addition and removal in one call, 200 entries at most; the Kit after it.
    pub fn change(&self, add: &[Key], remove: &[Key]) -> Result<(Kit, Option<String>), Failure> {
        let body = serde_json::json!({ "add": add, "remove": remove }).to_string();
        let answer = Call::post(self.service.api("/kit/changes")?.as_str(), "application/json", body).send(&self.service, Some(&self.token))?;
        Ok((parse(&answer.body)?, answer.etag))
    }
}

const FORM: &str = "application/x-www-form-urlencoded";

pub(crate) fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Camera => "camera",
        Kind::Lens => "lens",
    }
}

/// One request; never an `Origin` header, as the service refuses a write that carries one.
struct Call {
    url: String,
    post: Option<(&'static str, String)>,
    etag: Option<String>,
}

struct Answer {
    status: u16,
    etag: Option<String>,
    body: String,
}

impl Call {
    fn get(url: &str) -> Call {
        Call { url: url.to_string(), post: None, etag: None }
    }

    fn post(url: &str, content_type: &'static str, body: String) -> Call {
        Call { url: url.to_string(), post: Some((content_type, body)), etag: None }
    }

    /// The answer when 2xx or 304, else the failure its status stands for.
    fn send(self, service: &Service, token: Option<&str>) -> Result<Answer, Failure> {
        let work = async {
            let client = client(&service.agent)?;
            let mut request = match self.post {
                Some((kind, body)) => client.post(&self.url).header("Content-Type", kind).body(body),
                None => client.get(&self.url),
            };
            if let Some(token) = token {
                request = request.header("Authorization", format!("Bearer {token}"));
            }
            if let Some(etag) = self.etag {
                request = request.header("If-None-Match", etag);
            }
            let response = request.send().await.map_err(down)?;
            let status = response.status().as_u16();
            let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
            let (etag, wait) = (header("etag"), header("retry-after"));
            let body = response.text().await.map_err(down)?;
            match failure(status, wait.as_deref(), &body) {
                Some(failure) => Err(failure),
                None => Ok(Answer { status, etag, body }),
            }
        };
        tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(down)?.block_on(work)
    }
}

fn client(agent: &str) -> Result<reqwest::Client, Failure> {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    reqwest::Client::builder().user_agent(agent).timeout(TIMEOUT).build().map_err(down)
}

fn down(error: impl std::fmt::Display) -> Failure {
    Failure::Down(error.to_string())
}

/// The failure a status stands for, the service's own `{"error": …}` as its reason.
fn failure(status: u16, retry_after: Option<&str>, body: &str) -> Option<Failure> {
    #[derive(Deserialize)]
    struct Said {
        error: String,
    }
    let reason = || serde_json::from_str::<Said>(body).map(|s| s.error).unwrap_or_else(|_| format!("HTTP {status}"));
    match status {
        200..=299 | 304 => None,
        401 => Some(Failure::SignedOut),
        // A 429 without its delay waits the rate limit's whole minute.
        429 => Some(Failure::Wait(retry_after.and_then(|s| s.trim().parse().ok()).unwrap_or(60))),
        500.. => Some(Failure::Down(reason())),
        _ => Some(Failure::Refused(reason())),
    }
}

/// A 2xx body that does not parse is the service's fault, so the copy stands.
fn parse<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, Failure> {
    serde_json::from_str(body).map_err(down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_stand_for_their_failures() {
        assert_eq!(failure(200, None, ""), None);
        assert_eq!(failure(304, None, ""), None);
        assert_eq!(failure(401, None, ""), Some(Failure::SignedOut));
        assert_eq!(failure(429, Some("12"), ""), Some(Failure::Wait(12)));
        assert_eq!(failure(400, None, r#"{"error":"q too short"}"#), Some(Failure::Refused("q too short".into())));
        assert_eq!(failure(403, None, "nope"), Some(Failure::Refused("HTTP 403".into())));
        assert!(matches!(failure(503, None, ""), Some(Failure::Down(_))));
        // Twin: a 429 is never read as a refusal, nor a 5xx as signed out.
        assert_ne!(failure(429, Some("12"), ""), Some(Failure::Refused("HTTP 429".into())));
        assert_ne!(failure(500, None, ""), Some(Failure::SignedOut));
    }

    #[test]
    fn pages_carry_the_source_once() {
        let service = Service::new("https://thelensdb.com/", "1.7.0");
        assert!(service.agent.starts_with("ONI/1.7.0 ("), "{}", service.agent);
        assert_eq!(service.page("https://thelensdb.com/lenses/x"), "https://thelensdb.com/lenses/x?utm_source=oni");
        assert_eq!(service.page("https://thelensdb.com/lenses/x?a=1"), "https://thelensdb.com/lenses/x?a=1&utm_source=oni");
        assert_eq!(service.page("https://thelensdb.com/x?utm_source=oni"), "https://thelensdb.com/x?utm_source=oni");
        assert_eq!(service.submit_page(), "https://thelensdb.com/submit?utm_source=oni");
        // Twin: the source is never appended as a second `?`.
        assert!(!service.page("https://thelensdb.com/lenses/x?a=1").contains("?utm_source"));
    }
}
