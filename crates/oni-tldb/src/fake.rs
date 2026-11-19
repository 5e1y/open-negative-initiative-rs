//! A simulated The Lens DB on a loopback port, to the letter of its partner API v1: its sign-in and sign-up
//! pages, token, revocation, search, Kit and changes, its firewall's pages and its silences. The checks and
//! `ONI_TLDB=fake` use it, so nothing of ONI's tests ever reaches the real service.

mod accounts;
mod catalogue;
mod http;
mod journal;
mod look;
mod pages;
mod partner;

use crate::api::{Client, Service};
use crate::piece::{Key, Kind, Piece};
use accounts::Account;
use http::{Request, Response};
use std::collections::{HashMap, VecDeque};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub use catalogue::read as read_catalogue;
pub use journal::Seen;

/// A running simulated service; it stops with the process.
pub struct Fake {
    /// `http://127.0.0.1:<port>`, a `Service`'s site.
    pub site: String,
    state: Arc<Mutex<State>>,
}

/// How a simulated service starts.
#[derive(Default)]
pub struct Setup {
    /// 0 takes a free port.
    pub port: u16,
    /// The pieces it knows, else a small catalogue of its own.
    pub catalogue: Option<Vec<Piece>>,
    /// The file its accounts, Kits and tokens live in from one run to the next; none keeps them in memory.
    pub kept: Option<PathBuf>,
}

struct State {
    site: String,
    catalogue: Vec<Piece>,
    accounts: Vec<Account>,
    codes: HashMap<String, Code>,
    /// Each live token and its account.
    tokens: HashMap<String, Live>,
    /// Each account's calls inside the rate's window: the rate is the account's, all its tokens together.
    recent: HashMap<usize, VecDeque<Instant>>,
    /// A browser signed in on the pages, waiting on the consent page: its account.
    grants: HashMap<String, usize>,
    issued: u64,
    down: bool,
    /// A firewall's HTML page at this status in place of every API answer.
    firewall: Option<u16>,
    /// Every request read, then the connection closed without a word.
    silent: bool,
    calls: usize,
    /// So many calls an account may make in so long: the contract's 60 a minute.
    rate: (usize, Duration),
    journal: Vec<Seen>,
    kept: Option<PathBuf>,
}

struct Live {
    account: usize,
}

/// An authorisation code waiting for its exchange.
struct Code {
    account: usize,
    challenge: String,
    redirect: String,
    issued: Instant,
    used: bool,
}

/// Starts on a free port, over its own catalogue, keeping nothing.
pub fn start() -> Fake {
    start_with(Setup::default()).expect("a loopback port for the simulated The Lens DB")
}

pub fn start_with(setup: Setup) -> std::io::Result<Fake> {
    let listener = TcpListener::bind(("127.0.0.1", setup.port))?;
    let site = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let catalogue = setup.catalogue.unwrap_or_else(|| catalogue::pieces(&site));
    let mut state = State {
        accounts: vec![accounts::first(&catalogue)],
        catalogue,
        site: site.clone(),
        codes: HashMap::new(),
        tokens: HashMap::new(),
        recent: HashMap::new(),
        grants: HashMap::new(),
        issued: 0,
        down: false,
        firewall: None,
        silent: false,
        calls: 0,
        rate: (60, Duration::from_secs(60)),
        journal: Vec::new(),
        kept: setup.kept,
    };
    accounts::load(&mut state);
    let state = Arc::new(Mutex::new(state));
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

    /// A fresh token of the first account, as a finished sign-in would give.
    pub fn token(&self) -> String {
        lock(&self.state).issue_token(0)
    }

    /// A signed-in client, without the browser's round trip.
    pub fn client(&self) -> Client {
        Client { service: self.service(), token: self.token() }
    }

    /// A client of a second account, made with an empty Kit the first time it is asked for.
    pub fn stranger(&self) -> Client {
        let mut state = lock(&self.state);
        let account = accounts::find(&state, accounts::SECOND_EMAIL).unwrap_or_else(|| {
            state.accounts.push(accounts::second());
            state.accounts.len() - 1
        });
        let token = state.issue_token(account);
        Client { service: self.service(), token }
    }

    /// The consent page's choice made for the first account without a browser: the address the browser would
    /// be sent to, carrying a fresh code, or `error=access_denied` when `refuse`. Panics on a refused address.
    pub fn approve(&self, authorize_url: &str, refuse: bool) -> String {
        let url = url::Url::parse(authorize_url).expect("an authorisation address");
        let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        pages::decide(&mut lock(&self.state), &query, 0, refuse).unwrap_or_else(|reason| panic!("the authorisation page refuses: {reason}"))
    }

    /// Every token ends, as when the person changes their password.
    pub fn sign_out_everyone(&self) {
        let mut state = lock(&self.state);
        state.tokens.clear();
        state.keep();
    }

    /// While down, every address answers 503 with the API's error.
    pub fn set_down(&self, down: bool) {
        lock(&self.state).down = down;
    }

    /// While set, every address under `/api` answers a firewall's HTML page at that status, with no error of the API.
    pub fn set_firewall(&self, status: Option<u16>) {
        lock(&self.state).firewall = status;
    }

    /// While set, every connection is read, then closed without an answer.
    pub fn set_silent(&self, silent: bool) {
        lock(&self.state).silent = silent;
    }

    /// Two records found to be one: `from` leaves the catalogue, every Kit holding it holds `into` instead.
    pub fn merge(&self, from: Key, into: Key) {
        let mut state = lock(&self.state);
        state.catalogue.retain(|p| p.key() != from);
        for account in &mut state.accounts {
            let had = account.kit.contains(&from) && !account.kit.contains(&into);
            account.kit = account.kit.iter().map(|k| if *k == from && had { into } else { *k }).filter(|k| *k != from).collect();
        }
        state.keep();
    }

    /// Every request under `/api` received, in its order, with what it carried and its status.
    pub fn journal(&self) -> Vec<Seen> {
        lock(&self.state).journal.clone()
    }

    /// The partner calls received (search, Kit, changes), whatever they were answered.
    pub fn calls(&self) -> usize {
        lock(&self.state).calls
    }

    /// The first account's Kit, oldest first.
    pub fn kit_keys(&self) -> Vec<Key> {
        lock(&self.state).accounts[0].kit.clone()
    }

    /// So many calls an account may make in `window` before a 429; the contract's is 60 in a minute.
    pub fn set_rate(&self, calls: usize, window: Duration) {
        lock(&self.state).rate = (calls, window);
    }
}

impl State {
    fn issue_token(&mut self, account: usize) -> String {
        self.issued += 1;
        let token = format!("tldb_fake_{}", self.issued);
        self.tokens.insert(token.clone(), Live { account });
        self.keep();
        token
    }

    /// Writes the accounts and tokens to the kept file, when there is one.
    fn keep(&self) {
        accounts::save(self);
    }

    fn knows(&self, key: &Key) -> bool {
        self.catalogue.iter().any(|p| p.key() == *key)
    }
}

/// The first body and the first lens of a catalogue: the first account's Kit when it is created.
fn starting_kit(catalogue: &[Piece]) -> Vec<Key> {
    [Kind::Camera, Kind::Lens].iter().filter_map(|&kind| catalogue.iter().find(|p| p.kind == kind).map(Piece::key)).collect()
}

/// A thread that panicked under the lock left a state still worth serving.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn serve(mut stream: TcpStream, state: &Mutex<State>) {
    let Ok(request) = http::read(&stream) else { return };
    let mut state = lock(state);
    if state.silent {
        journal::note(&mut state, &request, 0);
        return;
    }
    let response = route(&mut state, &request);
    journal::note(&mut state, &request, response.status());
    drop(state);
    let _ = response.write(&mut stream);
}

fn route(state: &mut State, request: &Request) -> Response {
    let api = request.path.strip_prefix(crate::api::BASE);
    if matches!(api, Some("/search" | "/kit" | "/kit/changes")) {
        state.calls += 1;
    }
    if state.down {
        return Response::error(503, "the simulated service is down");
    }
    if request.path.starts_with("/api/") {
        if let Some(status) = state.firewall {
            return journal::firewall(status);
        }
        // The contract's firewall stops a call that does not name ONI with a check page, no error of the API.
        if !request.header("user-agent").is_some_and(journal::oni_agent) {
            return journal::firewall(429);
        }
    }
    if let Some(rest) = api {
        return partner::api(state, request, rest);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/oauth/authorize") => pages::authorize(request),
        ("POST", "/oauth/sign-in") => pages::sign_in(state, request),
        ("POST", "/oauth/sign-up") => pages::sign_up(state, request),
        ("GET", "/oauth/confirm") => pages::confirm(state, request),
        ("GET", "/oauth/quick") => pages::quick(state, request),
        ("GET", "/oauth/approve") => pages::approve(state, request),
        ("POST", "/api/oauth/token") => partner::token(state, request),
        ("POST", "/api/oauth/revoke") => partner::revoke(state, request),
        ("GET", "/submit") => pages::submit(),
        ("GET", "/") => pages::home(state),
        ("GET", path) => pages::piece(state, path),
        _ => Response::error(404, "no such address"),
    }
}
