//! The partner calls against the simulated service: search, the Kit and its ETag, changes, revocation,
//! a service down and the rate limit.

use crate::raw;
use oni_tldb::fake;
use oni_tldb::{Failure, Key, Kind, Read, Service};
use std::time::Duration;

const AE1: Key = Key { kind: Kind::Camera, id: 1101 };
const FD50: Key = Key { kind: Kind::Lens, id: 2101 };
const FM2: Key = Key { kind: Kind::Camera, id: 1103 };
const NIKKOR50: Key = Key { kind: Kind::Lens, id: 2107 };

fn names(pieces: &[oni_tldb::Piece]) -> Vec<&str> {
    pieces.iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn search_matches_every_word_in_any_order_case_and_accents_aside() {
    let fake = fake::start();
    let client = fake.client();
    let canons = client.search("canon", Some(Kind::Camera), None, 20).unwrap();
    assert_eq!(names(&canons), ["Canon AE-1 Program", "Canon A-1"]);
    let lenses = client.search("canon", Some(Kind::Lens), None, 20).unwrap();
    assert!(lenses.len() >= 4 && lenses.iter().all(|p| p.kind == Kind::Lens), "{:?}", names(&lenses));
    assert_eq!(names(&client.search("50mm NIKON", None, None, 20).unwrap()), ["Nikon Nikkor 50mm f/1.8 AI-S"]);
    let nikons = client.search("50mm", None, Some("nikon"), 20).unwrap();
    assert!(!nikons.is_empty() && nikons.iter().all(|p| p.brand.as_deref() == Some("Nikon")), "a brand restricts: {:?}", names(&nikons));
    // Twin: without the brand, other makers' 50mm come too.
    assert!(client.search("50mm", None, None, 20).unwrap().iter().any(|p| p.brand.as_deref() != Some("Nikon")));
    let accented = client.search("nokton VOIGTLANDER", None, None, 20).unwrap();
    assert_eq!(names(&accented), ["Voigtländer Nokton 40mm f/1.4"]);
    assert_eq!(client.search("Voigtländer nokton", None, None, 20).unwrap(), accented);
    // A piece the catalogue gives no brand is still found by its name.
    let planar = client.search("planar", None, None, 20).unwrap();
    assert_eq!(planar.len(), 1);
    assert_eq!(planar[0].brand, None);
    assert_eq!(client.search("summicron 50", None, None, 20).unwrap().len(), 2);
    // Twin: a word found in no name finds nothing, though every other word matches.
    assert!(client.search("nokton zeiss", None, None, 20).unwrap().is_empty());
}

#[test]
fn search_holds_its_bounds() {
    let fake = fake::start();
    let client = fake.client();
    assert_eq!(client.search("mm", Some(Kind::Lens), None, 3).unwrap().len(), 3);
    assert!(client.search("mm", Some(Kind::Lens), None, 50).unwrap().len() > 3);
    let refused = |r: Result<Vec<oni_tldb::Piece>, Failure>| matches!(r, Err(Failure::Refused(_)));
    assert!(refused(client.search("mm", None, None, 51)));
    assert!(refused(client.search("mm", None, None, 0)));
    assert!(refused(client.search("a", None, None, 20)));
    assert!(refused(client.search(&"x".repeat(201), None, None, 20)));
    // Words past the tenth are ignored, never refused.
    let ten = client.search("canon a e 1 p r o g r a", None, None, 20).unwrap();
    assert_eq!(client.search("canon a e 1 p r o g r a zzz", None, None, 20).unwrap(), ten);
    // Twin: each bound itself is accepted, and the same word within the ten narrows.
    assert!(client.search("ae", None, None, 50).is_ok());
    assert!(client.search(&"x".repeat(200), None, None, 1).is_ok());
    assert!(client.search("canon a e 1 p r o g r zzz", None, None, 20).unwrap().is_empty());
}

#[test]
fn the_kit_is_read_once_then_unchanged_by_its_etag() {
    let fake = fake::start();
    let client = fake.client();
    let Ok(Read::Fresh { kit, etag: Some(etag) }) = client.kit(None) else { panic!("no fresh Kit with its ETag") };
    assert_eq!(kit.items.iter().map(|p| p.key()).collect::<Vec<_>>(), [AE1, FD50]);
    assert_eq!(kit.account.display_name.as_deref(), Some("Camille"));
    assert_eq!(client.kit(Some(&etag)), Ok(Read::Same));
    // Twin: a stale ETag reads the Kit afresh.
    assert!(matches!(client.kit(Some("\"stale\"")), Ok(Read::Fresh { .. })));
    let (_, changed) = client.change(&[FM2], &[]).unwrap();
    assert_ne!(changed.as_deref(), Some(etag.as_str()));
    assert!(matches!(client.kit(Some(&etag)), Ok(Read::Fresh { .. })));
    assert_eq!(client.kit(changed.as_deref()), Ok(Read::Same));
}

#[test]
fn a_change_adds_and_removes_in_one_call_and_repeats_harmlessly() {
    let fake = fake::start();
    let client = fake.client();
    let (kit, etag) = client.change(&[FM2, NIKKOR50], &[FD50]).unwrap();
    assert_eq!(kit.items.iter().map(|p| p.key()).collect::<Vec<_>>(), [AE1, FM2, NIKKOR50]);
    assert!(kit.not_found.is_empty());
    assert_eq!(fake.kit_keys(), [AE1, FM2, NIKKOR50]);
    let (again, same) = client.change(&[FM2, NIKKOR50], &[FD50]).unwrap();
    assert_eq!(again.items, kit.items);
    assert_eq!(same, etag);
    let unknown = Key { kind: Kind::Lens, id: 999_999 };
    let (kit, _) = client.change(&[unknown], &[FM2]).unwrap();
    assert_eq!(kit.not_found, [unknown]);
    assert_eq!(fake.kit_keys(), [AE1, NIKKOR50]);
    // An id is unique within its kind only: a camera with a lens's id is unknown.
    let (kit, _) = client.change(&[Key { kind: Kind::Camera, id: NIKKOR50.id }], &[]).unwrap();
    assert_eq!(kit.not_found.len(), 1);
}

#[test]
fn a_revoked_token_signs_out_and_no_other() {
    let fake = fake::start();
    let (gone, kept) = (fake.client(), fake.client());
    fake.service().revoke(&gone.token).unwrap();
    assert_eq!(gone.kit(None), Err(Failure::SignedOut));
    assert_eq!(gone.search("canon", None, None, 5), Err(Failure::SignedOut));
    assert_eq!(gone.change(&[FM2], &[]), Err(Failure::SignedOut));
    assert!(fake.service().revoke("tldb_never_issued").is_ok());
    // Twin: the account's other token still reads its Kit, until everyone is signed out.
    assert!(kept.kit(None).is_ok());
    fake.sign_out_everyone();
    assert_eq!(kept.kit(None), Err(Failure::SignedOut));
}

#[test]
fn a_service_down_or_absent_is_down() {
    let fake = fake::start();
    let client = fake.client();
    fake.set_down(true);
    assert!(matches!(client.kit(None), Err(Failure::Down(_))));
    assert!(matches!(client.search("canon", None, None, 5), Err(Failure::Down(_))));
    let absent = oni_tldb::Client { service: Service::new("http://127.0.0.1:1", "0"), token: client.token.clone() };
    assert!(matches!(absent.kit(None), Err(Failure::Down(_))));
    // Twin: the same client reads again once the service is back.
    fake.set_down(false);
    assert!(client.kit(None).is_ok());
}

#[test]
fn past_sixty_calls_a_minute_the_service_says_how_long_to_wait() {
    let fake = fake::start();
    let client = fake.client();
    let before = fake.calls();
    for n in 0..60 {
        assert!(client.kit(None).is_ok(), "call {n}");
    }
    let Err(Failure::Wait(seconds)) = client.kit(None) else { panic!("no wait on the 61st call") };
    assert!((1..=60).contains(&seconds), "{seconds}");
    assert_eq!(fake.calls() - before, 61);
    // The rate is the account's: its second token is held back too; twin: another account's is not.
    assert!(matches!(fake.client().kit(None), Err(Failure::Wait(_))));
    assert!(fake.stranger().kit(None).is_ok());
    // A short window frees its calls once past.
    let other = fake::start();
    other.set_rate(2, Duration::from_millis(300));
    let short = other.client();
    assert!(short.kit(None).is_ok() && short.kit(None).is_ok());
    assert_eq!(short.kit(None), Err(Failure::Wait(1)));
    std::thread::sleep(Duration::from_millis(320));
    assert!(short.kit(None).is_ok());
}

#[test]
fn only_oni_without_an_origin_is_served() {
    let fake = fake::start();
    let token = fake.token();
    let kit = format!("{}/api/partner/v1/kit", fake.site);
    let changes = format!("{}/api/partner/v1/kit/changes", fake.site);
    let bearer = format!("Authorization: Bearer {token}");
    let body = r#"{"add":[{"type":"camera","id":1103}]}"#;
    // The firewall stops a call not naming ONI with an HTML page at 429, no error of the API, the token
    // and revocation addresses included.
    for (method, url) in [("GET", kit.clone()), ("POST", format!("{}/api/oauth/token", fake.site)), ("POST", format!("{}/api/oauth/revoke", fake.site))] {
        for agent in ["User-Agent: curl/8", "User-Agent: ONI/1", "X-None: 1"] {
            let answer = raw::request(method, &url, &[&bearer, agent], "");
            assert_eq!(answer.status, 429, "{method} {url} with {agent}");
            assert!(answer.head.contains("text/html") && !answer.body.contains("\"error\""), "{}", answer.body);
        }
    }
    let origin = raw::request("POST", &changes, &[&bearer, "User-Agent: ONI/1 (macOS)", "Origin: https://example.com"], body);
    assert_eq!(origin.status, 403);
    assert!(origin.body.contains("\"error\""));
    assert_eq!(fake.kit_keys(), [AE1, FD50]);
    // Twin: the same write as ONI sends it is applied.
    assert_eq!(raw::request("POST", &changes, &[&bearer, "User-Agent: ONI/1 (macOS)"], body).status, 200);
    assert_eq!(fake.kit_keys(), [AE1, FD50, FM2]);
}

#[test]
fn every_piece_links_to_a_page_naming_it() {
    let fake = fake::start();
    let client = fake.client();
    let mut pieces = client.search("mm", None, None, 50).unwrap();
    pieces.extend(client.search("contax", None, None, 5).unwrap());
    let compact = pieces.last().unwrap().clone();
    pieces.push(*compact.built_in_lens.expect("the T2's own lens"));
    for piece in &pieces {
        let page = raw::get(&fake.service().page(&piece.url));
        assert_eq!(page.status, 200, "{}", piece.url);
        assert!(page.body.contains("simulated"));
        assert!(page.body.contains(&piece.name.replace('&', "&amp;")), "{}", piece.name);
    }
    assert!(raw::get(&fake.service().submit_page()).body.contains("Propose a piece"));
    // Twin: an address no piece has is no page.
    assert_eq!(raw::get(&format!("{}/lenses/no-such-lens", fake.site)).status, 404);
}
