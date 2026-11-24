//! The simulated pages as a browser meets them: sign-in, sign-up and its confirmation e-mail, the consent,
//! each account's own Kit, the accounts kept from one run to the next, and a catalogue read from a file.

use crate::raw::{self, Answer};
use oni_fixtures::scratch;
use oni_tldb::fake::{self, Fake, Setup};
use oni_tldb::oauth::{self, Outcome, Pending};
use oni_tldb::{Client, Failure, Key, Kind, Read};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// The value that follows `marker` up to the next quote, unescaped.
fn after(page: &Answer, marker: &str) -> String {
    let start = page.body.find(marker).unwrap_or_else(|| panic!("no {marker} in {}", page.body)) + marker.len();
    unescape(page.body[start..].split('"').next().unwrap_or_default())
}

fn post(fake: &Fake, path: &str, fields: &[(&str, &str)]) -> Answer {
    let body = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(fields).finish();
    raw::request("POST", &format!("{}{path}", fake.site), &["Content-Type: application/x-www-form-urlencoded"], &body)
}

/// Signs in on the page and authorises ONI: the address the browser is sent back to.
fn sign_in(fake: &Fake, oauth: &str, email: &str, password: &str) -> String {
    allow(fake, &post(fake, "/oauth/sign-in", &[("oauth", oauth), ("email", email), ("password", password)]))
}

/// Authorises ONI on a consent page: the address the browser is sent back to.
fn allow(fake: &Fake, consent: &Answer) -> String {
    assert!(consent.body.contains("id=\"allow\""), "{}", consent.body);
    let allowed = raw::get(&format!("{}{}", fake.site, after(consent, "id=\"allow\" href=\"")));
    assert_eq!(allowed.status, 302, "{}", allowed.body);
    allowed.head.lines().find_map(|l| l.strip_prefix("Location: ")).expect("a redirect").to_string()
}

/// The loopback answering the browser's return.
fn token(fake: &Fake, pending: Pending, back: String) -> String {
    let browser = std::thread::spawn(move || raw::get(&back));
    let outcome = pending.finish(&fake.service(), Duration::from_secs(10), &AtomicBool::new(false), &|_| "<p>back</p>".into());
    browser.join().unwrap();
    let Outcome::Token(token) = outcome else { panic!("no token: {outcome:?}") };
    token
}

#[test]
fn the_pages_sign_in_then_ask_before_authorising() {
    let fake = fake::start();
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let page = raw::get(&pending.url);
    assert!(page.body.contains("Simulated.") && page.body.contains("<h1>Sign in</h1>"), "{}", page.body);
    let oauth = after(&page, "name=\"oauth\" value=\"");
    let wrong = post(&fake, "/oauth/sign-in", &[("oauth", &oauth), ("email", "camille@example.com"), ("password", "wrong")]);
    assert!(wrong.body.contains("does not match") && !wrong.body.contains("id=\"allow\""), "twin: a wrong password reaches no consent");
    let back = sign_in(&fake, &oauth, " Camille@Example.com ", "film");
    assert!(back.starts_with(&format!("{}?code=", pending.redirect)), "{back}");
    let client = Client { service: fake.service(), token: token(&fake, pending, back) };
    let Ok(Read::Fresh { kit, .. }) = client.kit(None) else { panic!("no Kit") };
    assert_eq!(kit.account.display_name.as_deref(), Some("Camille"));
    // A refusal on the consent sends the browser back with access_denied.
    let pending = oauth::begin(&fake.service(), false).unwrap();
    let oauth = after(&raw::get(&pending.url), "name=\"oauth\" value=\"");
    let consent = post(&fake, "/oauth/sign-in", &[("oauth", &oauth), ("email", "camille@example.com"), ("password", "film")]);
    let refuse = format!("{}{}", fake.site, after(&consent, "id=\"refuse\" href=\""));
    assert!(raw::get(&refuse).head.contains("error=access_denied"));
    assert_eq!(raw::get(&refuse).status, 400, "a consent is answered once");
    // The test account's one click signs in and leads to the consent, from the sign-in page as from the sign-up one.
    for signup in [false, true] {
        let pending = oauth::begin(&fake.service(), signup).unwrap();
        let quick = raw::get(&format!("{}{}", fake.site, after(&raw::get(&pending.url), "id=\"quick\" href=\"")));
        assert_eq!(quick.status, 200, "twin: the one click stops at the consent, never straight back to ONI: {}", quick.head);
        assert!(!quick.head.contains("Location:") && quick.body.contains("Camille"), "{}", quick.head);
        assert!(allow(&fake, &quick).starts_with(&format!("{}?code=", pending.redirect)));
    }
    // Twin: another client is refused before any form.
    assert_eq!(raw::get(&pending.url.replace("client_id=oni", "client_id=other")).status, 400);
}

#[test]
fn a_new_account_confirms_its_e_mail_and_keeps_its_own_kit_from_run_to_run() {
    let kept = scratch("tldb-kept").join("tldb-fake.json");
    let fake = fake::start_with(Setup { kept: Some(kept.clone()), ..Setup::default() }).unwrap();
    let pending = oauth::begin(&fake.service(), true).unwrap();
    let page = raw::get(&pending.url);
    assert!(page.body.contains("Create your account"), "{}", page.body);
    let oauth = after(&page, "name=\"oauth\" value=\"");
    let fields = [("oauth", oauth.as_str()), ("name", "Lou"), ("email", "lou@example.com"), ("password", "x")];
    let mail = post(&fake, "/oauth/sign-up", &fields);
    assert!(mail.body.contains("Check your e-mail"), "{}", mail.body);
    assert!(post(&fake, "/oauth/sign-up", &fields).body.contains("already uses"), "one account an e-mail");
    let early = post(&fake, "/oauth/sign-in", &[("oauth", &oauth), ("email", "lou@example.com"), ("password", "x")]);
    assert!(early.body.contains("Confirm your e-mail before") && !early.body.contains("id=\"allow\""), "twin: no consent before the e-mail");
    let confirmed = raw::get(&format!("{}{}", fake.site, after(&mail, "id=\"confirm\" href=\"")));
    assert!(confirmed.body.contains("Your e-mail is confirmed"), "{}", confirmed.body);
    let back = sign_in(&fake, &after(&confirmed, "name=\"oauth\" value=\""), "lou@example.com", "x");
    let lou = Client { service: fake.service(), token: token(&fake, pending, back) };
    let Ok(Read::Fresh { kit, .. }) = lou.kit(None) else { panic!("no Kit") };
    assert_eq!((kit.account.display_name.as_deref(), kit.items.len()), (Some("Lou"), 0));
    let fm2 = Key { kind: Kind::Camera, id: 1103 };
    lou.change(&[fm2], &[]).unwrap();
    assert_eq!(fake.kit_keys().len(), 2, "Camille's Kit untouched");
    // Another run over the same file knows Lou, the token and the Kit.
    let again = fake::start_with(Setup { kept: Some(kept), ..Setup::default() }).unwrap();
    let lou = Client { service: again.service(), token: lou.token };
    let Ok(Read::Fresh { kit, .. }) = lou.kit(None) else { panic!("the token forgotten") };
    assert_eq!(kit.items.iter().map(|p| p.key()).collect::<Vec<_>>(), [fm2]);
    // Twin: a run that keeps nothing knows no such token.
    let forgetful = fake::start();
    assert_eq!(Client { service: forgetful.service(), token: lou.token }.kit(None), Err(Failure::SignedOut));
}

#[test]
fn a_catalogue_file_replaces_the_simulated_one() {
    let file = scratch("tldb-catalogue").join("catalogue.json");
    let pieces = r#"[
        {"type":"camera","id":996,"name":"Nikon FM2","brand":"Nikon","model":"FM2","url":"https://thelensdb.com/cameras/nikon-fm2-1982","mount":"Nikon F"},
        {"type":"lens","id":2691,"name":"Leica APO-Summicron-M 90mm F/2 ASPH. [V]","brand":"Leica","model":"APO-Summicron-M 90mm F/2 ASPH. [V]",
         "url":"https://thelensdb.com/lenses/leica-apo-summicron-m-90mm-f2-asph-v-1998","mounts":["Leica M"],
         "focalLengthMm":{"min":90,"max":90},"maxAperture":{"atMinFocal":2,"atMaxFocal":2}}
    ]"#;
    std::fs::write(&file, pieces).unwrap();
    let catalogue = fake::read_catalogue(&file).unwrap();
    let fake = fake::start_with(Setup { catalogue: Some(catalogue), ..Setup::default() }).unwrap();
    let client = fake.client();
    assert_eq!(client.search("summicron", None, None, 20).unwrap()[0].id, 2691);
    assert_eq!(fake.kit_keys(), [Key { kind: Kind::Camera, id: 996 }, Key { kind: Kind::Lens, id: 2691 }], "its first body and lens");
    // Twin: the simulated catalogue's own pieces are gone.
    assert!(client.search("pentax mx", None, None, 20).unwrap().is_empty());
}
