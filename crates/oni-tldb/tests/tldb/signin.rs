//! The sign-in end to end: the authorisation address, the page's choice, the browser's return to the
//! loopback, and the code exchanged for a token.

use crate::raw;
use oni_tldb::Read;
use oni_tldb::fake::{self, Fake};
use oni_tldb::oauth::{self, Outcome, Pending};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

fn page(signed_in: bool) -> String {
    if signed_in { "<p>back to ONI</p>".into() } else { "<p>ONI was refused</p>".into() }
}

/// The browser following `urls` in turn, each answer kept.
fn browse(urls: Vec<String>) -> JoinHandle<Vec<raw::Answer>> {
    std::thread::spawn(move || urls.iter().map(|u| raw::get(u)).collect())
}

fn finish(fake: &Fake, pending: Pending) -> Outcome {
    pending.finish(&fake.service(), Duration::from_secs(10), &AtomicBool::new(false), &page)
}

#[test]
fn a_sign_in_returns_a_token_that_reads_the_kit() {
    let fake = fake::start();
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let back = fake.approve(&pending.url, false);
    assert!(back.starts_with(&pending.redirect), "{back}");
    let browser = browse(vec![back]);
    let Outcome::Token(token) = finish(&fake, pending) else { panic!("no token") };
    let answers = browser.join().unwrap();
    assert_eq!(answers[0].status, 200);
    assert!(answers[0].body.contains("back to ONI"));
    let client = oni_tldb::Client { service: fake.service(), token };
    let Ok(Read::Fresh { kit, .. }) = client.kit(None) else { panic!("the token reads no Kit") };
    assert_eq!(kit.account.display_name, "Camille");
    // Twin: a made-up token reads nothing.
    let stranger = oni_tldb::Client { service: fake.service(), token: "tldb_made_up".into() };
    assert_eq!(stranger.kit(None), Err(oni_tldb::Failure::SignedOut));
}

#[test]
fn a_refusal_on_the_page_ends_refused() {
    let fake = fake::start();
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let back = fake.approve(&pending.url, true);
    assert!(back.contains("error=access_denied"), "{back}");
    let browser = browse(vec![back]);
    assert_eq!(finish(&fake, pending), Outcome::Refused);
    assert!(browser.join().unwrap()[0].body.contains("ONI was refused"));
    // Twin: the same steps approved end with a token.
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let browser = browse(vec![fake.approve(&pending.url, false)]);
    assert!(matches!(finish(&fake, pending), Outcome::Token(_)));
    browser.join().unwrap();
}

#[test]
fn a_stray_request_never_ends_the_sign_in() {
    let fake = fake::start();
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let back = fake.approve(&pending.url, false);
    let port = pending.redirect.trim_end_matches("/callback").to_string();
    let stale = format!("{port}/callback?code=code_1&state=an-older-tab");
    let refused = format!("{port}/callback?error=access_denied&state=an-older-tab");
    let browser = browse(vec![stale, refused, format!("{port}/favicon.ico"), format!("{port}/callback"), back]);
    let outcome = finish(&fake, pending);
    let statuses: Vec<u16> = browser.join().unwrap().iter().map(|a| a.status).collect();
    assert_eq!(statuses, [400, 400, 404, 400, 200]);
    // Twin: the request carrying this sign-in's state is the one that ended it, with a token.
    assert!(matches!(outcome, Outcome::Token(_)), "{outcome:?}");
}

#[test]
fn a_sign_in_ends_on_cancel_or_on_its_time() {
    let fake = fake::start();
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        flag.store(true, Ordering::Relaxed);
    });
    assert_eq!(pending.finish(&fake.service(), Duration::from_secs(10), &cancel, &page), Outcome::Cancelled);
    canceller.join().unwrap();
    let pending = oauth::begin(&fake.service(), false).unwrap();
    assert_eq!(pending.finish(&fake.service(), Duration::from_millis(120), &AtomicBool::new(false), &page), Outcome::TimedOut);
    // Twin: a return that comes in time is no timeout.
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let browser = browse(vec![fake.approve(&pending.url, false)]);
    let outcome = pending.finish(&fake.service(), Duration::from_secs(10), &AtomicBool::new(false), &page);
    assert!(matches!(outcome, Outcome::Token(_)), "{outcome:?}");
    browser.join().unwrap();
}
