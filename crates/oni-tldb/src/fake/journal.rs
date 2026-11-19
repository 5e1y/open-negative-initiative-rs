//! What the simulated service saw of each API call, and the firewall's page the contract warns of: HTML at
//! any status, with no error of the API.

use super::State;
use super::http::{Request, Response};
use std::time::Instant;

/// One request under `/api`, as it came; `status` 0 when the service kept silent.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub agent: Option<String>,
    pub bearer: Option<String>,
    pub origin: Option<String>,
    pub status: u16,
    pub at: Instant,
}

impl Seen {
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

pub fn note(state: &mut State, request: &Request, status: u16) {
    if !request.path.starts_with("/api/") {
        return;
    }
    let header = |name: &str| request.header(name).map(str::to_string);
    let bearer = request.header("authorization").and_then(|v| v.strip_prefix("Bearer ")).map(str::to_string);
    state.journal.push(Seen {
        method: request.method.clone(),
        path: request.path.clone(),
        query: request.query.clone(),
        agent: header("user-agent"),
        bearer,
        origin: header("origin"),
        status,
        at: Instant::now(),
    });
}

/// `ONI/<version> (<system>)`, each part present.
pub fn oni_agent(agent: &str) -> bool {
    let Some((version, rest)) = agent.strip_prefix("ONI/").and_then(|r| r.split_once(" (")) else { return false };
    let system = rest.strip_suffix(')').unwrap_or_default();
    !version.is_empty() && !version.contains(' ') && !system.is_empty() && !system.contains(['(', ')'])
}

pub fn firewall(status: u16) -> Response {
    Response::html(status, "<!doctype html><title>Checking your browser</title><p>One moment, please.</p>".into())
}
