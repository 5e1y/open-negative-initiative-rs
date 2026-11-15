//! A simulated The Lens DB on a loopback port, as its contract states it: its sign-in page, token,
//! revocation, search, Kit and changes, over a small invented catalogue. The checks and `ONI_TLDB=fake` use it.

mod catalogue;
mod http;
mod pages;
mod partner;

use crate::api::{Client, Service};
use crate::piece::{Key, Piece};
use http::{Request, Response};
use std::collections::{HashMap, VecDeque};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A running simulated service; it stops with the process.
pub struct Fake {
    /// `http://127.0.0.1:<port>`, a `Service`'s site.
    pub site: String,
    state: Arc<Mutex<State>>,
}

/// The one account, its Kit shared by every token it was given.
struct State {
    site: String,
    catalogue: Vec<Piece>,
    kit: Vec<Key>,
    codes: HashMap<String, Code>,
    /// Each live token and the instants of its calls inside the rate's window.
    tokens: HashMap<String, VecDeque<Instant>>,
    issued: u64,
    down: bool,
    calls: usize,
    /// So many calls a token may make in so long: the contract's 60 a minute.
    rate: (usize, Duration),
}

/// An authorisation code waiting for its exchange.
struct Code {
    challenge: String,
    redirect: String,
    issued: Instant,
    used: bool,
}

/// The account's name, the same on every start.
const ACCOUNT: &str = "Camille";

/// Starts on a free port.
pub fn start() -> Fake {
    start_on(0).expect("a loopback port for the simulated The Lens DB")
}

/// Starts on `port`, so a person's browser finds it at the same address each time.
pub fn start_on(port: u16) -> std::io::Result<Fake> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let site = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let state = Arc::new(Mutex::new(State {
        catalogue: catalogue::pieces(&site),
        site: site.clone(),
        kit: catalogue::STARTING_KIT.to_vec(),
        codes: HashMap::new(),
        tokens: HashMap::new(),
        issued: 0,
        down: false,
        calls: 0,
        rate: (60, Duration::from_secs(60)),
    }));
    let shared = state.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = shared.clone();
            std::thread::spawn(move || serve(stream, &state));
        }
    });
    Ok(Fake { site, state })
}

impl Fake {
    /// The service as ONI names itself to it.
    pub fn service(&self) -> Service {
        Service::new(&self.site, env!("CARGO_PKG_VERSION"))
    }

    /// A fresh token of the account, as a finished sign-in would give.
    pub fn token(&self) -> String {
        lock(&self.state).issue_token()
    }

    /// A signed-in client, without the browser's round trip.
    pub fn client(&self) -> Client {
        Client { service: self.service(), token: self.token() }
    }

    /// The authorisation page's choice made without a browser: the address the browser would be sent to,
    /// carrying a fresh code, or `error=access_denied` when `refuse`. Panics on an address the page refuses.
    pub fn approve(&self, authorize_url: &str, refuse: bool) -> String {
        let url = url::Url::parse(authorize_url).expect("an authorisation address");
        let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        pages::decide(&mut lock(&self.state), &query, refuse).unwrap_or_else(|reason| panic!("the authorisation page refuses: {reason}"))
    }

    /// Every token ends, as when the person changes their password.
    pub fn sign_out_everyone(&self) {
        lock(&self.state).tokens.clear();
    }

    /// While down, every address answers 503.
    pub fn set_down(&self, down: bool) {
        lock(&self.state).down = down;
    }

    /// The partner calls received (search, Kit, changes), whatever they were answered.
    pub fn calls(&self) -> usize {
        lock(&self.state).calls
    }

    /// The account's Kit, oldest first.
    pub fn kit_keys(&self) -> Vec<Key> {
        lock(&self.state).kit.clone()
    }

    /// So many calls a token may make in `window` before a 429; the contract's is 60 in a minute.
    pub fn set_rate(&self, calls: usize, window: Duration) {
        lock(&self.state).rate = (calls, window);
    }
}

impl State {
    fn issue_token(&mut self) -> String {
        self.issued += 1;
        let token = format!("tldb_fake_{}", self.issued);
        self.tokens.insert(token.clone(), VecDeque::new());
        token
    }
}

/// A thread that panicked under the lock left a state still worth serving.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn serve(mut stream: TcpStream, state: &Mutex<State>) {
    if let Ok(request) = http::read(&stream) {
        let _ = route(state, &request).write(&mut stream);
    }
}

fn route(state: &Mutex<State>, request: &Request) -> Response {
    let mut state = lock(state);
    let api = request.path.strip_prefix(crate::api::BASE);
    if matches!(api, Some("/search" | "/kit" | "/kit/changes")) {
        state.calls += 1;
    }
    if state.down {
        return Response::error(503, "the simulated service is down");
    }
    if let Some(rest) = api {
        return partner::api(&mut state, request, rest);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/oauth/authorize") => pages::authorize(request),
        ("GET", "/oauth/approve") => pages::approve(&mut state, request),
        ("POST", "/api/oauth/token") => partner::token(&mut state, request),
        ("POST", "/api/oauth/revoke") => partner::revoke(&mut state, request),
        ("GET", "/submit") => pages::submit(),
        ("GET", path) => pages::piece(&state, path),
        _ => Response::error(404, "no such address"),
    }
}
