//! The simulated service's pages: the sign-in or sign-up page that authorises ONI, a piece's page and the
//! submission form. Plainly styled and bannered, so none can pass for the real site.

use super::catalogue::slug;
use super::http::{Request, Response, escape};
use super::{ACCOUNT, Code, State};
use std::time::Instant;

/// What an authorisation address asks, once its parameters hold.
struct Asked {
    redirect: String,
    state: String,
    challenge: String,
    signup: bool,
}

/// The contract's parameters; the first that does not hold is the refusal's reason.
fn asked(query: &[(String, String)]) -> Result<Asked, String> {
    let value = |name: &str| query.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str()).unwrap_or_default();
    let expect = |name: &str, wanted: &str| if value(name) == wanted { Ok(()) } else { Err(format!("{name} must be {wanted}")) };
    expect("client_id", crate::CLIENT_ID)?;
    expect("response_type", "code")?;
    expect("scope", "kit")?;
    expect("code_challenge_method", "S256")?;
    let redirect = value("redirect_uri");
    let port = redirect.strip_prefix("http://127.0.0.1:").and_then(|r| r.strip_suffix("/callback"));
    if !port.is_some_and(|p| p.parse::<u16>().is_ok()) {
        return Err("redirect_uri must be http://127.0.0.1:<port>/callback".into());
    }
    let challenge = value("code_challenge");
    let base64url = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if !(43..=128).contains(&challenge.len()) || !challenge.chars().all(base64url) {
        return Err("code_challenge must be 43 to 128 base64url characters".into());
    }
    if value("state").is_empty() {
        return Err("state is missing".into());
    }
    Ok(Asked { redirect: redirect.into(), state: value("state").into(), challenge: challenge.into(), signup: value("screen_hint") == "signup" })
}

pub fn authorize(request: &Request) -> Response {
    let asked = match asked(&request.query) {
        Ok(asked) => asked,
        Err(reason) => return refused(&reason),
    };
    let choice = |decision: &str| {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.extend_pairs(&request.query).append_pair("decision", decision);
        escape(&format!("/oauth/approve?{}", query.finish()))
    };
    let (title, lead) = if asked.signup {
        ("Create your account", format!("A new account, <b>{ACCOUNT}</b>, is created at once: no e-mail to confirm here."))
    } else {
        ("Sign in", format!("Signed in as <b>{ACCOUNT}</b>."))
    };
    let body = format!(
        "<h1>{title}</h1><p>{lead}</p><p>ONI asks to read and change your Kit, nothing else.</p>\
         <p><a class=\"go\" href=\"{}\">Authorize ONI</a> <a href=\"{}\">Refuse</a></p>",
        choice("allow"),
        choice("refuse")
    );
    Response::html(200, shell(title, &body))
}

pub fn approve(state: &mut State, request: &Request) -> Response {
    match decide(state, &request.query, request.param("decision") != Some("allow")) {
        Ok(location) => Response::redirect(location),
        Err(reason) => refused(&reason),
    }
}

/// The address the browser returns to: a fresh code, or the refusal, with the sign-in's own state.
pub fn decide(state: &mut State, query: &[(String, String)], refuse: bool) -> Result<String, String> {
    let asked = asked(query)?;
    let mut back = url::Url::parse(&asked.redirect).map_err(|e| e.to_string())?;
    if refuse {
        back.query_pairs_mut().append_pair("error", "access_denied").append_pair("state", &asked.state);
        return Ok(back.into());
    }
    state.issued += 1;
    let code = format!("code_{}", state.issued);
    state.codes.insert(code.clone(), Code { challenge: asked.challenge, redirect: asked.redirect, issued: Instant::now(), used: false });
    back.query_pairs_mut().append_pair("code", &code).append_pair("state", &asked.state);
    Ok(back.into())
}

/// `/cameras/<slug>` and `/lenses/<slug>`, from the catalogue and the compacts' own lenses.
pub fn piece(state: &State, path: &str) -> Response {
    let mut all = state.catalogue.iter().flat_map(|p| std::iter::once(p).chain(p.built_in_lens.as_deref()));
    let found = all.find(|p| p.url.strip_prefix(state.site.as_str()) == Some(path));
    let Some(piece) = found else {
        return Response::html(404, shell("Not found", "<h1>Not found</h1><p>No piece of the simulated catalogue lives here.</p>"));
    };
    let mut facts = Vec::new();
    if let Some(brand) = &piece.brand {
        facts.push(format!("Brand: {}", escape(brand)));
    }
    if let Some(mount) = &piece.mount {
        facts.push(format!("Mount: {}", escape(mount)));
    }
    if let Some(mounts) = piece.mounts.as_ref().filter(|m| !m.is_empty()) {
        facts.push(format!("Mounts: {}", escape(&mounts.join(", "))));
    }
    if let Some(year) = piece.year_introduced {
        facts.push(format!("Introduced: {year}"));
    }
    if let Some(own) = &piece.built_in_lens {
        facts.push(format!("Lens: <a href=\"{}\">{}</a>", escape(&own.url), escape(&own.name)));
    }
    let list: String = facts.iter().map(|f| format!("<li>{f}</li>")).collect();
    let body = format!("<h1>{}</h1><p class=\"id\">{} {}, page {}</p><ul>{list}</ul>", escape(&piece.name), kind(piece), piece.id, escape(&slug(&piece.name)));
    Response::html(200, shell(&piece.name, &body))
}

fn kind(piece: &crate::Piece) -> &'static str {
    crate::api::kind_name(piece.kind)
}

pub fn submit() -> Response {
    let body = "<h1>Propose a piece</h1><p>The real site's form, reviewed before publication, stands here. \
                Nothing is sent from this page.</p><form><p><input placeholder=\"Brand and name\" disabled></p></form>";
    Response::html(200, shell("Propose a piece", body))
}

fn refused(reason: &str) -> Response {
    Response::html(400, shell("Refused", &format!("<h1>This request is refused</h1><p>{}</p>", escape(reason))))
}

/// Every page: the banner first, readable at 500 points wide.
fn shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>The Lens DB — simulated: {}</title><style>\
         body{{font:16px/1.5 system-ui,sans-serif;max-width:32rem;margin:0 auto;padding:1rem;background:#fff;color:#222}}\
         .sim{{background:#ffe9a8;border:2px dashed #a06b00;padding:.75rem;margin-bottom:1.5rem}}\
         a{{color:#0645ad}}a.go{{font-weight:bold;margin-right:1.5rem}}.id{{color:#666}}\
         </style></head><body><div class=\"sim\"><b>The Lens DB — simulated.</b> A local simulation for ONI's \
         development on this computer: not the real site, no real account.</div>{body}</body></html>",
        escape(title)
    )
}
