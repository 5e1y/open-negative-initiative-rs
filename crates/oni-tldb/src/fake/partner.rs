//! The simulated service's token, revocation and partner calls, with the contract's checks: the agent,
//! the bearer token, the rate, no `Origin` on a write, and the bounds of a search and of a change.

use super::catalogue::fold;
use super::http::{Request, Response};
use super::{ACCOUNT, State};
use crate::piece::{Key, Kind};
use serde::Deserialize;
use std::time::{Duration, Instant};

/// A code is exchanged within this long, once.
const CODE_LIFE: Duration = Duration::from_secs(120);

/// The most entries one change carries.
const CHANGES: usize = 200;

pub fn token(state: &mut State, request: &Request) -> Response {
    let form = request.form();
    let value = |name: &str| form.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str()).unwrap_or_default();
    if value("grant_type") != "authorization_code" {
        return Response::error(400, "unsupported_grant_type");
    }
    if value("client_id") != crate::CLIENT_ID {
        return Response::error(400, "invalid_client");
    }
    let Some(code) = state.codes.get_mut(value("code")) else { return Response::error(400, "invalid_grant") };
    let proven = crate::oauth::challenge(value("code_verifier")) == code.challenge;
    if code.used || code.issued.elapsed() > CODE_LIFE || code.redirect != value("redirect_uri") || !proven {
        return Response::error(400, "invalid_grant");
    }
    code.used = true;
    let token = state.issue_token();
    Response::json(200, &serde_json::json!({ "access_token": token, "token_type": "Bearer", "scope": "kit" }))
}

pub fn revoke(state: &mut State, request: &Request) -> Response {
    if let Some((_, token)) = request.form().into_iter().find(|(n, _)| n == "token") {
        state.tokens.remove(&token);
    }
    Response::empty(200)
}

pub fn api(state: &mut State, request: &Request, path: &str) -> Response {
    if !request.header("user-agent").is_some_and(|agent| agent.starts_with("ONI/")) {
        return Response::error(403, "unknown client");
    }
    let token = request.header("authorization").and_then(|v| v.strip_prefix("Bearer ")).unwrap_or_default();
    let (most, window) = state.rate;
    let Some(recent) = state.tokens.get_mut(token) else { return Response::error(401, "invalid_token") };
    let now = Instant::now();
    while recent.front().is_some_and(|&at| now.duration_since(at) >= window) {
        recent.pop_front();
    }
    if recent.len() >= most {
        let free = recent.front().map_or(window, |&at| window.saturating_sub(now.duration_since(at)));
        let seconds = free.as_secs() + u64::from(free.subsec_nanos() > 0);
        return Response::error(429, "rate_limited").with("Retry-After", seconds.max(1).to_string());
    }
    recent.push_back(now);
    if request.method == "POST" && request.header("origin").is_some() {
        return Response::error(403, "a write carrying an Origin is refused");
    }
    match (request.method.as_str(), path) {
        ("GET", "/search") => search(state, request),
        ("GET", "/kit") => {
            let etag = etag(&state.kit);
            if request.header("if-none-match") == Some(etag.as_str()) {
                return Response::empty(304).with("ETag", etag);
            }
            Response::json(200, &kit(state, None)).with("ETag", etag)
        }
        ("POST", "/kit/changes") => change(state, request),
        _ => Response::error(404, "no such call"),
    }
}

fn search(state: &State, request: &Request) -> Response {
    let q = request.param("q").unwrap_or_default().trim();
    let words: Vec<String> = q.split_whitespace().map(fold).collect();
    if !(2..=200).contains(&q.chars().count()) || words.len() > 10 {
        return Response::error(400, "q must be 2 to 200 characters and 10 words at most");
    }
    let kind = match request.param("type") {
        None => None,
        Some("camera") => Some(Kind::Camera),
        Some("lens") => Some(Kind::Lens),
        Some(_) => return Response::error(400, "type must be camera or lens"),
    };
    let limit = match request.param("limit").map(str::parse::<usize>) {
        None => 20,
        Some(Ok(n)) if (1..=50).contains(&n) => n,
        Some(_) => return Response::error(400, "limit must be 1 to 50"),
    };
    let brand = request.param("brand").map(fold);
    let results: Vec<_> = state
        .catalogue
        .iter()
        .filter(|p| kind.is_none_or(|k| p.kind == k))
        .filter(|p| brand.as_ref().is_none_or(|b| p.brand.as_deref().map(fold).as_ref() == Some(b)))
        .filter(|p| {
            let haystack = fold(&format!("{} {}", p.brand.as_deref().unwrap_or_default(), p.name));
            words.iter().all(|w| haystack.contains(w.as_str()))
        })
        .take(limit)
        .collect();
    Response::json(200, &serde_json::json!({ "results": results }))
}

#[derive(Deserialize)]
struct Changes {
    #[serde(default)]
    add: Vec<Key>,
    #[serde(default)]
    remove: Vec<Key>,
}

fn change(state: &mut State, request: &Request) -> Response {
    let Ok(changes) = serde_json::from_slice::<Changes>(&request.body) else { return Response::error(400, "malformed change") };
    if changes.add.len() + changes.remove.len() > CHANGES {
        return Response::error(400, "200 entries at most");
    }
    let known = |key: &Key| state.catalogue.iter().any(|p| p.key() == *key);
    let mut not_found = Vec::new();
    for key in &changes.remove {
        if known(key) {
            state.kit.retain(|k| k != key);
        } else {
            not_found.push(*key);
        }
    }
    for key in &changes.add {
        if !known(key) {
            not_found.push(*key);
        } else if !state.kit.contains(key) {
            state.kit.push(*key);
        }
    }
    Response::json(200, &kit(state, Some(not_found))).with("ETag", etag(&state.kit))
}

/// The Kit as the service answers it, oldest first.
fn kit(state: &State, not_found: Option<Vec<Key>>) -> serde_json::Value {
    let items: Vec<_> = state.kit.iter().filter_map(|key| state.catalogue.iter().find(|p| p.key() == *key)).collect();
    let mut answer = serde_json::json!({ "account": { "displayName": ACCOUNT }, "items": items });
    if let Some(not_found) = not_found {
        answer["notFound"] = serde_json::json!(not_found);
    }
    answer
}

/// Named by the Kit's content, so a copy from an earlier run of the simulation is never taken as current.
fn etag(kit: &[Key]) -> String {
    let bytes = serde_json::to_vec(kit).unwrap_or_default();
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let hex: String = digest.as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}
