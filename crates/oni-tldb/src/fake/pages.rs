//! The simulated service's pages: sign-in, sign-up with its simulated confirmation e-mail, the consent that
//! authorises ONI, a piece's page and the submission form. Bannered, so none can pass for the real site.

use super::accounts::{self, Account};
use super::http::{Request, Response};
use super::look::{self, Form};
use super::{Code, State};
use std::time::Instant;

/// What an authorisation address asks, once its parameters hold.
struct Asked {
    redirect: String,
    state: String,
    challenge: String,
    signup: bool,
    /// The parameters again, carried by the pages' forms and links until the consent.
    query: String,
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
    const CARRIED: [&str; 9] =
        ["response_type", "client_id", "redirect_uri", "scope", "state", "code_challenge", "code_challenge_method", "screen_hint", "utm_source"];
    let mut carried = url::form_urlencoded::Serializer::new(String::new());
    carried.extend_pairs(query.iter().filter(|(n, _)| CARRIED.contains(&n.as_str())));
    Ok(Asked {
        redirect: redirect.into(),
        state: value("state").into(),
        challenge: challenge.into(),
        signup: value("screen_hint") == "signup",
        query: carried.finish(),
    })
}

/// A form's fields, and the authorisation it carries in its `oauth` field.
fn posted(request: &Request) -> (Vec<(String, String)>, Result<Asked, String>) {
    let form = request.form();
    let oauth = form.iter().find(|(n, _)| n == "oauth").map(|(_, v)| v.clone()).unwrap_or_default();
    let query: Vec<(String, String)> = url::form_urlencoded::parse(oauth.as_bytes()).into_owned().collect();
    (form, asked(&query))
}

fn field<'a>(form: &'a [(String, String)], name: &str) -> &'a str {
    form.iter().find(|(n, _)| n == name).map(|(_, v)| v.trim()).unwrap_or_default()
}

pub fn authorize(request: &Request) -> Response {
    match asked(&request.query) {
        Ok(asked) if asked.signup => look::sign_up(&asked.query, &Form::default()),
        Ok(asked) => look::sign_in(&asked.query, &Form::default()),
        Err(reason) => look::refused(&reason),
    }
}

pub fn sign_in(state: &mut State, request: &Request) -> Response {
    let (form, asked) = posted(request);
    let asked = match asked {
        Ok(asked) => asked,
        Err(reason) => return look::refused(&reason),
    };
    let (email, password) = (field(&form, "email"), field(&form, "password"));
    let wrong = |error: &str| look::sign_in(&asked.query, &Form { email: email.into(), error: error.into(), ..Form::default() });
    let Some(index) = accounts::find(state, email) else { return wrong("No account uses this e-mail. Create one below.") };
    let account = &state.accounts[index];
    if account.password != password {
        return wrong("The password does not match this e-mail.");
    }
    if let Some(link) = &account.unconfirmed {
        return look::mailbox(&account.email, &confirm_link(link, &asked.query), true);
    }
    let grant = crate::oauth::random(16).unwrap_or_default();
    state.grants.insert(grant.clone(), index);
    look::consent(&state.accounts[index], &approve_link(&asked.query, &grant, "allow"), &approve_link(&asked.query, &grant, "refuse"))
}

pub fn sign_up(state: &mut State, request: &Request) -> Response {
    let (form, asked) = posted(request);
    let asked = match asked {
        Ok(asked) => asked,
        Err(reason) => return look::refused(&reason),
    };
    let (name, email, password) = (field(&form, "name"), field(&form, "email").to_lowercase(), field(&form, "password"));
    let again = |error: &str| look::sign_up(&asked.query, &Form { name: name.into(), email: email.clone(), error: error.into(), ..Form::default() });
    if name.is_empty() || password.is_empty() || !email.contains('@') {
        return again("A name, an e-mail and a password, please.");
    }
    if accounts::find(state, &email).is_some() {
        return again("An account already uses this e-mail. Sign in instead.");
    }
    let link = crate::oauth::random(16).unwrap_or_default();
    let account = Account { email: email.clone(), password: password.into(), name: name.into(), unconfirmed: Some(link.clone()), kit: Vec::new() };
    state.accounts.push(account);
    state.keep();
    look::mailbox(&email, &confirm_link(&link, &asked.query), false)
}

/// The simulated e-mail's link: it confirms the account, then the sign-in goes on.
pub fn confirm(state: &mut State, request: &Request) -> Response {
    let query: Vec<(String, String)> = url::form_urlencoded::parse(request.param("oauth").unwrap_or_default().as_bytes()).into_owned().collect();
    let asked = match asked(&query) {
        Ok(asked) => asked,
        Err(reason) => return look::refused(&reason),
    };
    let link = request.param("link").unwrap_or_default();
    let Some(account) = state.accounts.iter_mut().find(|a| a.unconfirmed.as_deref() == Some(link)) else {
        return look::refused("This confirmation link was already used, or never sent.");
    };
    account.unconfirmed = None;
    let email = account.email.clone();
    state.keep();
    let notice = "Your e-mail is confirmed. Sign in to continue to ONI.";
    look::sign_in(&asked.query, &Form { email, notice: notice.into(), ..Form::default() })
}

fn confirm_link(link: &str, query: &str) -> String {
    let mut pairs = url::form_urlencoded::Serializer::new(String::new());
    pairs.append_pair("link", link).append_pair("oauth", query);
    format!("/oauth/confirm?{}", pairs.finish())
}

fn approve_link(query: &str, grant: &str, decision: &str) -> String {
    let mut pairs = url::form_urlencoded::Serializer::new(query.to_string());
    pairs.append_pair("grant", grant).append_pair("decision", decision);
    format!("/oauth/approve?{}", pairs.finish())
}

/// The test account's one click: the first account authorises ONI without a form.
pub fn quick(state: &mut State, request: &Request) -> Response {
    match decide(state, &request.query, 0, false) {
        Ok(location) => Response::redirect(location),
        Err(reason) => look::refused(&reason),
    }
}

pub fn approve(state: &mut State, request: &Request) -> Response {
    let Some(account) = request.param("grant").and_then(|g| state.grants.remove(g)) else {
        return look::refused("This consent was already answered, or nobody signed in.");
    };
    match decide(state, &request.query, account, request.param("decision") != Some("allow")) {
        Ok(location) => Response::redirect(location),
        Err(reason) => look::refused(&reason),
    }
}

/// The address the browser returns to: a fresh code of `account`, or the refusal, with the sign-in's state.
pub fn decide(state: &mut State, query: &[(String, String)], account: usize, refuse: bool) -> Result<String, String> {
    let asked = asked(query)?;
    let mut back = url::Url::parse(&asked.redirect).map_err(|e| e.to_string())?;
    if refuse {
        back.query_pairs_mut().append_pair("error", "access_denied").append_pair("state", &asked.state);
        return Ok(back.into());
    }
    state.issued += 1;
    let code = format!("code_{}", state.issued);
    let entry = Code { account, challenge: asked.challenge, redirect: asked.redirect, issued: Instant::now(), used: false };
    state.codes.insert(code.clone(), entry);
    back.query_pairs_mut().append_pair("code", &code).append_pair("state", &asked.state);
    Ok(back.into())
}

pub fn home(state: &State) -> Response {
    look::home(state.catalogue.iter().filter(|p| p.kind == crate::Kind::Camera).count(), state.catalogue.len())
}

/// A piece of the simulated catalogue whose address lies on this site, a compact's own lens included.
pub fn piece(state: &State, path: &str) -> Response {
    let mut all = state.catalogue.iter().flat_map(|p| std::iter::once(p).chain(p.built_in_lens.as_deref()));
    match all.find(|p| p.url.strip_prefix(state.site.as_str()) == Some(path)) {
        Some(piece) => look::piece(piece),
        None => Response::html(404, look::shell("Not found", "<h1>Not found</h1><p>No piece of the simulated catalogue lives here.</p>")),
    }
}

pub fn submit() -> Response {
    let body = "<h1>Propose a piece</h1><p>The real site's form, reviewed before publication, stands here. \
                Nothing is sent from this page.</p><form><p><input placeholder=\"Brand and name\" disabled></p></form>";
    Response::html(200, look::shell("Propose a piece", body))
}
