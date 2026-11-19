//! The partner API v1's letter, each point against the simulated service: what every call carries, the
//! batches of a change, the ETag, a 401, a 429's delay, a firewall's page, a silence, the token address's
//! errors, a merged record and the fields the service may add or leave empty.

use crate::raw;
use oni_tldb::fake::{self, Fake};
use oni_tldb::oauth;
use oni_tldb::{Client, Failure, Key, Kind, Read, Service};
use std::time::Duration;

const AE1: Key = Key { kind: Kind::Camera, id: 1101 };
const FD50: Key = Key { kind: Kind::Lens, id: 2101 };
const FM2: Key = Key { kind: Kind::Camera, id: 1103 };

/// A sign-in through the token address, as ONI makes it.
fn signed_in(fake: &Fake) -> Client {
    let service = fake.service();
    let pending = oauth::begin(&service, false).unwrap();
    let back = fake.approve(&pending.url, false);
    let browser = std::thread::spawn(move || raw::get(&back));
    let outcome = pending.finish(&service, Duration::from_secs(10), &std::sync::atomic::AtomicBool::new(false), &|_| String::new());
    browser.join().unwrap();
    let oauth::Outcome::Token(token) = outcome else { panic!("no token: {outcome:?}") };
    Client { service, token }
}

#[test]
fn every_call_names_oni_and_its_system_bears_the_token_and_no_origin() {
    let fake = fake::start();
    let client = signed_in(&fake);
    client.search("canon", None, None, 5).unwrap();
    client.kit(None).unwrap();
    client.change(&[FM2], &[]).unwrap();
    client.service.revoke(&client.token).unwrap();
    let journal = fake.journal();
    let paths: Vec<&str> = journal.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(paths, ["/api/oauth/token", "/api/partner/v1/search", "/api/partner/v1/kit", "/api/partner/v1/kit/changes", "/api/oauth/revoke"]);
    for seen in &journal {
        let agent = seen.agent.as_deref().unwrap_or_default();
        let system = agent.strip_prefix(&format!("ONI/{} (", env!("CARGO_PKG_VERSION"))).and_then(|r| r.strip_suffix(')'));
        assert!(matches!(system, Some("macOS" | "Windows" | "linux")), "a plain system's name, nothing of the machine: {agent}");
        assert_eq!(seen.origin, None, "{}", seen.path);
        assert_eq!(seen.bearer.is_some(), seen.path != "/api/oauth/token", "{}", seen.path);
    }
    // Twin: a call naming another program meets the firewall's page, which ONI reads as the service unreached.
    let curl = raw::request("GET", &format!("{}/api/partner/v1/kit", fake.site), &["User-Agent: curl/8"], "");
    assert_eq!(curl.status, 429);
    let foreign = Client { service: Service { site: fake.site.clone(), agent: "curl/8".into() }, token: fake.token() };
    assert!(matches!(foreign.kit(None), Err(Failure::Down(_))));
    // Twin: a write carrying an Origin is refused with the API's error, which ONI reads as its own fault.
    let token = fake.token();
    let bearer = format!("Authorization: Bearer {token}");
    let body = r#"{"add":[{"type":"camera","id":1103}]}"#;
    let origin = raw::request("POST", &format!("{}/api/partner/v1/kit/changes", fake.site), &[&bearer, "User-Agent: ONI/1 (macOS)", "Origin: https://example.com"], body);
    assert_eq!(origin.status, 403);
    assert!(origin.body.contains("\"error\""));
}

#[test]
fn a_long_change_goes_in_calls_of_two_hundred_removals_first() {
    let fake = fake::start();
    let client = fake.client();
    let unknown: Vec<Key> = (0..199).map(|id| Key { kind: Kind::Lens, id: 900_000 + id }).collect();
    let adds: Vec<Key> = unknown.iter().copied().chain([FM2, FD50]).collect();
    let (kit, _) = client.change(&adds, &[FD50]).unwrap();
    let posts: Vec<_> = fake.journal().into_iter().filter(|s| s.path.ends_with("/kit/changes")).collect();
    assert_eq!(posts.len(), 2, "202 entries in two calls");
    assert_eq!(kit.items.iter().map(|p| p.key()).collect::<Vec<_>>(), [AE1, FM2, FD50], "a key removed and added ends present");
    assert_eq!(kit.not_found.len(), 199, "every call's unknown ids, the first's included");
    // Twin: adds before removes would leave the key absent, as the service applies removals last only within a call.
    let other = fake::start().client();
    other.change(&[FD50], &[]).unwrap();
    let (kit, _) = other.change(&[], &[FD50]).unwrap();
    assert!(!kit.items.iter().any(|p| p.key() == FD50));
    // Twin: one call of 201 entries is refused by the service itself.
    let too_many: Vec<String> = (0..201).map(|id| format!(r#"{{"type":"lens","id":{id}}}"#)).collect();
    let body = format!(r#"{{"add":[{}]}}"#, too_many.join(","));
    let bearer = format!("Authorization: Bearer {}", fake.token());
    assert_eq!(raw::request("POST", &format!("{}/api/partner/v1/kit/changes", fake.site), &[&bearer, "User-Agent: ONI/1 (macOS)"], &body).status, 400);
}

#[test]
fn a_weak_etag_is_sent_back_whole() {
    let fake = fake::start();
    let client = fake.client();
    let Ok(Read::Fresh { etag: Some(etag), .. }) = client.kit(None) else { panic!("no ETag") };
    assert!(etag.starts_with("W/\""), "{etag}");
    assert_eq!(client.kit(Some(&etag)), Ok(Read::Same));
    // Twin: the same tag stripped of its weakness reads afresh.
    assert!(matches!(client.kit(Some(etag.trim_start_matches("W/"))), Ok(Read::Fresh { .. })));
}

#[test]
fn a_429_waits_its_retry_after_and_a_revoked_token_signs_out_everywhere() {
    let fake = fake::start();
    let client = fake.client();
    fake.set_rate(1, Duration::from_secs(7));
    client.kit(None).unwrap();
    let wait = client.search("canon", None, None, 5);
    assert_eq!(fake.journal().last().map(|s| s.status), Some(429));
    assert!(matches!(wait, Err(Failure::Wait(6 | 7))), "{wait:?}");
    let fresh = fake::start();
    let gone = fresh.client();
    fresh.service().revoke(&gone.token).unwrap();
    assert_eq!(gone.kit(None), Err(Failure::SignedOut));
    assert_eq!(gone.search("canon", None, None, 5), Err(Failure::SignedOut));
    assert_eq!(gone.change(&[FM2], &[]).map(|_| ()), Err(Failure::SignedOut));
    // Twin: the account's other token still reads.
    assert!(fresh.client().kit(None).is_ok());
}

#[test]
fn a_firewalls_page_or_a_silence_is_the_service_unreached() {
    let fake = fake::start();
    let client = fake.client();
    for status in [403, 429, 200, 503] {
        fake.set_firewall(Some(status));
        assert!(matches!(client.kit(None), Err(Failure::Down(_))), "a firewall's {status}");
        assert!(matches!(client.change(&[FM2], &[]), Err(Failure::Down(_))), "a firewall's {status}");
    }
    fake.set_firewall(None);
    fake.set_silent(true);
    assert!(matches!(client.search("canon", None, None, 5), Err(Failure::Down(_))));
    // Twin: the same client reads once the service answers again.
    fake.set_silent(false);
    assert!(client.kit(None).is_ok());
    assert_eq!(fake.kit_keys(), [AE1, FD50], "nothing a firewall stopped was applied");
}

#[test]
fn the_token_address_answers_rfc_6749_errors() {
    let fake = fake::start();
    let token = format!("{}/api/oauth/token", fake.site);
    let agent = "User-Agent: ONI/1 (macOS)";
    let form = "Content-Type: application/x-www-form-urlencoded";
    let post = |body: &str| raw::request("POST", &token, &[agent, form], body);
    let unknown = post("grant_type=authorization_code&code=x&redirect_uri=http%3A%2F%2F127.0.0.1%3A1%2Fcallback&client_id=other&code_verifier=v");
    assert_eq!(unknown.status, 401);
    assert!(unknown.body.contains("\"invalid_client\"") && unknown.body.contains("error_description"), "{}", unknown.body);
    assert_eq!(post("grant_type=authorization_code&client_id=oni").status, 400);
    assert!(post("grant_type=authorization_code&client_id=oni").body.contains("invalid_request"));
    assert!(post("grant_type=password&code=x&redirect_uri=r&client_id=oni&code_verifier=v").body.contains("unsupported_grant_type"));
    // Twin: the right fields go through to the code's own check.
    assert!(post("grant_type=authorization_code&code=x&redirect_uri=r&client_id=oni&code_verifier=v").body.contains("invalid_grant"));
}

#[test]
fn a_merged_record_comes_back_as_the_one_kept() {
    let fake = fake::start();
    let client = fake.client();
    let a1 = Key { kind: Kind::Camera, id: 1102 };
    client.change(&[a1], &[]).unwrap();
    fake.merge(a1, FM2);
    let Ok(Read::Fresh { kit, .. }) = client.kit(None) else { panic!("no fresh Kit after the merge") };
    let keys: Vec<Key> = kit.items.iter().map(|p| p.key()).collect();
    assert_eq!(keys, [AE1, FD50, FM2]);
    // Twin: the merged-away record is gone from the catalogue.
    assert!(client.search("canon a-1", None, None, 5).unwrap().is_empty());
}

#[test]
fn unknown_fields_are_ignored_and_empty_ones_read() {
    let item = r#"{"type":"camera","id":1,"name":"Leica M6","brand":null,"model":"M6","url":"u","mount":null,"yearIntroduced":null,"builtInLens":null,"rating":4.5}"#;
    let kit: oni_tldb::Kit = serde_json::from_str(&format!(r#"{{"account":{{"displayName":null}},"items":[{item}],"cursor":"x"}}"#)).unwrap();
    assert_eq!((kit.account.display_name, kit.items[0].brand.clone()), (None, None));
    // Twin: a piece without its name is no piece.
    assert!(serde_json::from_str::<oni_tldb::Piece>(&item.replace(r#""name":"Leica M6","#, "")).is_err());
}
