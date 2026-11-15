//! The sign-in: OAuth 2's authorisation code with PKCE (S256), returned to a loopback port on 127.0.0.1.

use crate::api::{Failure, Service};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long the loopback waits: a new account confirms its e-mail before it can sign in.
pub const WAIT: Duration = Duration::from_secs(15 * 60);

/// A sign-in under way: the address to open in the browser and the port its return lands on.
pub struct Pending {
    pub url: String,
    pub redirect: String,
    verifier: String,
    state: String,
    listener: TcpListener,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Token(String),
    /// The person refused ONI on the service's page.
    Refused,
    Cancelled,
    TimedOut,
    Failed(Failure),
}

/// How often the loopback looks for the browser's return and for a cancel.
const POLL: Duration = Duration::from_millis(50);

/// A connection that never completes its request (a browser's preconnect) is dropped after this.
const IDLE: Duration = Duration::from_secs(10);

/// A request head larger than this is no browser's return.
const HEAD: usize = 16 * 1024;

/// A free loopback port, a fresh verifier and state, and the authorisation address; `signup` opens the
/// account's creation first.
pub fn begin(service: &Service, signup: bool) -> std::io::Result<Pending> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let redirect = format!("http://127.0.0.1:{}/callback", listener.local_addr()?.port());
    let verifier = random(32)?;
    let state = random(16)?;
    let mut url = url::Url::parse(&format!("{}/oauth/authorize", service.site)).map_err(std::io::Error::other)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("response_type", "code")
            .append_pair("client_id", crate::CLIENT_ID)
            .append_pair("redirect_uri", &redirect)
            .append_pair("scope", "kit")
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge(&verifier))
            .append_pair("code_challenge_method", "S256")
            .append_pair("utm_source", "oni");
        if signup {
            pairs.append_pair("screen_hint", "signup");
        }
    }
    listener.set_nonblocking(true)?;
    Ok(Pending { url: url.into(), redirect, verifier, state, listener })
}

/// PKCE's S256: the verifier's SHA-256, base64url without padding.
pub(crate) fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()))
}

fn random(bytes: usize) -> std::io::Result<String> {
    use ring::rand::SecureRandom;
    let mut buffer = vec![0u8; bytes];
    ring::rand::SystemRandom::new().fill(&mut buffer).map_err(|_| std::io::Error::other("no system randomness"))?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}

/// What one request on the loopback asks.
enum Return {
    Code(String),
    Error(String),
    /// Another path, or a request with no state of this sign-in: answered, and the wait goes on.
    Stray(u16),
}

impl Pending {
    /// Waits for the browser's return until `until` or `cancel`, exchanges its code for the token, then
    /// answers it with `page(signed_in)` as HTML.
    pub fn finish(self, service: &Service, until: Duration, cancel: &AtomicBool, page: &dyn Fn(bool) -> String) -> Outcome {
        let start = Instant::now();
        // Any browser may return, the default one after an e-mail's link included, so every connection counts.
        let mut open: Vec<(TcpStream, Vec<u8>, Instant)> = Vec::new();
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Outcome::Cancelled;
            }
            if start.elapsed() >= until {
                return Outcome::TimedOut;
            }
            while let Ok((stream, _)) = self.listener.accept() {
                if stream.set_nonblocking(true).is_ok() {
                    open.push((stream, Vec::new(), Instant::now()));
                }
            }
            let mut index = 0;
            while index < open.len() {
                let (stream, head, opened) = &mut open[index];
                match read_head(stream, head) {
                    Ok(true) => {
                        let (mut stream, head, _) = open.swap_remove(index);
                        match self.judge(&head) {
                            Return::Stray(status) => answer(&mut stream, status, "text/plain; charset=utf-8", reason(status)),
                            Return::Error(error) => {
                                answer(&mut stream, 200, "text/html; charset=utf-8", &page(false));
                                return if error == "access_denied" { Outcome::Refused } else { Outcome::Failed(Failure::Refused(error)) };
                            }
                            // The code exchanged before the page answers, so the browser says what ONI ends with.
                            Return::Code(code) => {
                                let outcome = match service.exchange(&code, &self.redirect, &self.verifier) {
                                    Ok(token) => Outcome::Token(token),
                                    Err(failure) => Outcome::Failed(failure),
                                };
                                answer(&mut stream, 200, "text/html; charset=utf-8", &page(matches!(outcome, Outcome::Token(_))));
                                return outcome;
                            }
                        }
                    }
                    Ok(false) if opened.elapsed() < IDLE => index += 1,
                    _ => {
                        open.swap_remove(index);
                    }
                }
            }
            std::thread::sleep(POLL);
        }
    }

    /// Only `GET /callback` with this sign-in's state ends the wait.
    fn judge(&self, head: &[u8]) -> Return {
        let text = String::from_utf8_lossy(head);
        let mut words = text.lines().next().unwrap_or_default().split(' ');
        let (method, target) = (words.next().unwrap_or_default(), words.next().unwrap_or_default());
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if method != "GET" || path != "/callback" {
            return Return::Stray(404);
        }
        let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
        let value = |name: &str| pairs.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
        if value("state").as_deref() != Some(self.state.as_str()) {
            return Return::Stray(400);
        }
        match (value("code"), value("error")) {
            (_, Some(error)) => Return::Error(error),
            (Some(code), None) => Return::Code(code),
            (None, None) => Return::Stray(400),
        }
    }
}

/// Reads what has come; true once the request's head is whole. An error or a closed or oversized
/// connection is dropped by the caller.
fn read_head(stream: &mut TcpStream, head: &mut Vec<u8>) -> std::io::Result<bool> {
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                head.extend_from_slice(&buffer[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    return Ok(true);
                }
                if head.len() > HEAD {
                    return Err(std::io::ErrorKind::InvalidData.into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) => return Err(e),
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    }
}

/// A browser that left before its answer loses nothing: the sign-in goes on from what it sent.
fn answer(stream: &mut TcpStream, status: u16, kind: &str, body: &str) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        reason(status),
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(body.as_bytes())).and_then(|_| stream.flush());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_is_the_verifiers_sha_256_in_base64url() {
        // Computed apart, by Python's hashlib and base64.urlsafe_b64encode without its padding.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r-wW1gFWlOnjFM"), "EYzJZRGixl_-3z-tWBHjLGGFhJntTeR43T9TYU9alZA");
        // Twin: a verifier one character off gives another challenge.
        assert_ne!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r-wW1gFWlOnjFN"), "EYzJZRGixl_-3z-tWBHjLGGFhJntTeR43T9TYU9alZA");
    }

    #[test]
    fn the_token_endpoint_wants_the_challenges_verifier() {
        let fake = crate::fake::start();
        let service = fake.service();
        let pending = begin(&service, false).unwrap();
        let back = url::Url::parse(&fake.approve(&pending.url, false)).unwrap();
        let code = back.query_pairs().find(|(n, _)| n == "code").map(|(_, v)| v.into_owned()).unwrap();
        let wrong = random(32).unwrap();
        assert!(matches!(service.exchange(&code, &pending.redirect, &wrong), Err(Failure::Refused(_))));
        let elsewhere = "http://127.0.0.1:1/callback";
        assert!(matches!(service.exchange(&code, elsewhere, &pending.verifier), Err(Failure::Refused(_))));
        // Twin: the verifier behind the challenge gets the token, once only.
        let token = service.exchange(&code, &pending.redirect, &pending.verifier).unwrap();
        assert!(token.starts_with("tldb_"), "{token}");
        assert!(matches!(service.exchange(&code, &pending.redirect, &pending.verifier), Err(Failure::Refused(_))));
    }

    #[test]
    fn the_authorisation_address_carries_every_parameter() {
        let service = crate::Service::new("https://thelensdb.com", "1.0");
        let pending = begin(&service, true).unwrap();
        let url = url::Url::parse(&pending.url).unwrap();
        let value = |name: &str| url.query_pairs().find(|(n, _)| n == name).map(|(_, v)| v.into_owned());
        assert_eq!(url.path(), "/oauth/authorize");
        assert_eq!(value("redirect_uri").as_deref(), Some(pending.redirect.as_str()));
        assert_eq!(value("code_challenge"), Some(challenge(&pending.verifier)));
        assert_eq!(value("state").as_deref(), Some(pending.state.as_str()));
        assert_eq!(value("screen_hint").as_deref(), Some("signup"));
        assert_eq!(value("utm_source").as_deref(), Some("oni"));
        assert_eq!(pending.verifier.len(), 43);
        // Twin: a sign-in that is no sign-up names no screen, and two sign-ins share no secret.
        let other = begin(&service, false).unwrap();
        assert!(!other.url.contains("screen_hint"));
        assert_ne!(other.verifier, pending.verifier);
        assert_ne!(other.state, pending.state);
    }
}
