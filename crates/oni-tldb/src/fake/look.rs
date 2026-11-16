//! The simulated pages' HTML: one plain shell under a banner that says what they are, and the sign-in,
//! sign-up, mailbox and consent cards.

use super::accounts::{Account, FIRST_EMAIL, FIRST_PASSWORD};
use super::http::{Response, escape};
use crate::piece::Piece;

/// What a sign-in or sign-up form shows again: the typed values and a line above them.
#[derive(Default)]
pub struct Form {
    pub name: String,
    pub email: String,
    pub error: String,
    pub notice: String,
}

pub fn sign_in(oauth: &str, form: &Form) -> Response {
    let signup = format!("/oauth/authorize?{}&screen_hint=signup", oauth.replace("&screen_hint=signup", ""));
    let body = format!(
        "<h1>Sign in</h1><p class=\"lead\">ONI would like to use your Kit.</p>{}\
         <form method=\"post\" action=\"/oauth/sign-in\">{}\
         <label>E-mail<input name=\"email\" type=\"email\" autocomplete=\"off\" value=\"{}\" required autofocus></label>\
         <label>Password<input name=\"password\" type=\"password\" required></label>\
         <button>Sign in</button></form>\
         <p class=\"aside\">No account yet? <a href=\"{}\">Create one</a></p>\
         <p class=\"hint\">Simulated account: <b>{FIRST_EMAIL}</b>, password <b>{FIRST_PASSWORD}</b>.</p>",
        lines(form),
        hidden(oauth),
        escape(&form.email),
        escape(&signup)
    );
    Response::html(200, shell("Sign in", &body))
}

pub fn sign_up(oauth: &str, form: &Form) -> Response {
    let signin = format!("/oauth/authorize?{}", oauth.replace("&screen_hint=signup", ""));
    let body = format!(
        "<h1>Create your account</h1><p class=\"lead\">Then ONI can keep your Kit here.</p>{}\
         <form method=\"post\" action=\"/oauth/sign-up\">{}\
         <label>Name<input name=\"name\" autocomplete=\"off\" value=\"{}\" required autofocus></label>\
         <label>E-mail<input name=\"email\" type=\"email\" autocomplete=\"off\" value=\"{}\" required></label>\
         <label>Password<input name=\"password\" type=\"password\" required></label>\
         <button>Create the account</button></form>\
         <p class=\"hint\">Invent a password: this simulation keeps it in plain text on this computer.</p>\
         <p class=\"aside\">Already registered? <a href=\"{}\">Sign in</a></p>",
        lines(form),
        hidden(oauth),
        escape(&form.name),
        escape(&form.email),
        escape(&signin)
    );
    Response::html(200, shell("Create your account", &body))
}

/// After a sign-up, or a sign-in before the confirmation: the e-mail the real site would send, shown here.
pub fn mailbox(email: &str, link: &str, again: bool) -> Response {
    let lead = if again { "Confirm your e-mail before signing in." } else { "Your account is created. Confirm your e-mail to sign in." };
    let body = format!(
        "<h1>Check your e-mail</h1><p class=\"lead\">{lead}</p>\
         <div class=\"mail\"><p class=\"from\">Simulated e-mail to <b>{}</b></p><p>Welcome to The Lens DB. Confirm this address \
         to use your account.</p><p><a class=\"button\" href=\"{}\">Confirm my e-mail</a></p></div>\
         <p class=\"hint\">ONI waits up to 15 minutes for this sign-in to end.</p>",
        escape(email),
        escape(link)
    );
    Response::html(200, shell("Check your e-mail", &body))
}

pub fn consent(account: &Account, allow: &str, refuse: &str) -> Response {
    let body = format!(
        "<h1>Authorize ONI</h1><p class=\"lead\">Signed in as <b>{}</b> · {}</p>\
         <div class=\"mail\"><p><b>ONI</b>, the film negative converter, asks to:</p>\
         <ul><li>read your Kit of bodies and lenses</li><li>add pieces to it and remove them</li></ul>\
         <p>Nothing else: not your notes, prices or serial numbers.</p></div>\
         <p class=\"choice\"><a class=\"button\" href=\"{}\">Authorize ONI</a><a class=\"quiet\" href=\"{}\">Refuse</a></p>",
        escape(&account.name),
        escape(&account.email),
        escape(allow),
        escape(refuse)
    );
    Response::html(200, shell("Authorize ONI", &body))
}

pub fn refused(reason: &str) -> Response {
    Response::html(400, shell("Refused", &format!("<h1>This request is refused</h1><p>{}</p>", escape(reason))))
}

pub fn home(cameras: usize, pieces: usize) -> Response {
    let body = format!(
        "<h1>The Lens DB</h1><p class=\"lead\">{cameras} cameras and {} lenses in this simulation. ONI searches them \
         once signed in.</p>",
        pieces - cameras
    );
    Response::html(200, shell("Home", &body))
}

pub fn piece(piece: &Piece) -> Response {
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
    let kind = crate::api::kind_name(piece.kind);
    let body = format!("<h1>{}</h1><p class=\"lead\">{kind} {}</p><ul>{list}</ul>", escape(&piece.name), piece.id);
    Response::html(200, shell(&piece.name, &body))
}

fn hidden(oauth: &str) -> String {
    format!("<input type=\"hidden\" name=\"oauth\" value=\"{}\">", escape(oauth))
}

fn lines(form: &Form) -> String {
    let mut out = String::new();
    if !form.notice.is_empty() {
        out.push_str(&format!("<p class=\"notice\">{}</p>", escape(&form.notice)));
    }
    if !form.error.is_empty() {
        out.push_str(&format!("<p class=\"error\">{}</p>", escape(&form.error)));
    }
    out
}

/// Every page: the banner first, one card readable at 360 points wide, light or dark as the system is.
pub fn shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>The Lens DB — simulated: {}</title><style>{STYLE}</style></head><body>\
         <div class=\"sim\"><b>Simulated.</b> A local stand-in for The Lens DB, for ONI's development on this computer: \
         not the real site, no real account.</div><header><a href=\"/\">THE LENS DB</a><span>simulated</span></header>\
         <main>{body}</main></body></html>",
        escape(title)
    )
}

const STYLE: &str = "\
:root{--ink:#111;--soft:#666;--line:#e3e3e3;--card:#fff;--ground:#f6f6f4;--go:#111;--on-go:#fff;--warn:#fff3cd}\
@media (prefers-color-scheme:dark){:root{--ink:#eee;--soft:#999;--line:#333;--card:#1b1b1b;--ground:#0e0e0e;--go:#eee;--on-go:#111;--warn:#3a3000}}\
*{box-sizing:border-box}body{margin:0;background:var(--ground);color:var(--ink);font:16px/1.5 system-ui,-apple-system,sans-serif}\
.sim{background:var(--warn);padding:.6rem 1rem;font-size:.85rem;text-align:center}\
header{display:flex;gap:.6rem;align-items:baseline;justify-content:center;padding:1.5rem 1rem .5rem}\
header a{color:var(--ink);font-weight:800;letter-spacing:.04em;text-decoration:none}header span{color:var(--soft);font-size:.8rem}\
main{max-width:26rem;margin:1rem auto 3rem;padding:2rem;background:var(--card);border:1px solid var(--line);border-radius:12px}\
h1{font-size:1.5rem;margin:0 0 .25rem}.lead{color:var(--soft);margin:0 0 1.5rem}\
label{display:block;font-size:.85rem;font-weight:600;margin-bottom:1rem}\
input{display:block;width:100%;margin-top:.35rem;padding:.65rem .75rem;font:inherit;color:var(--ink);background:var(--ground);\
border:1px solid var(--line);border-radius:8px}\
button,.button{display:inline-block;width:100%;padding:.7rem 1rem;font:inherit;font-weight:600;text-align:center;\
color:var(--on-go);background:var(--go);border:0;border-radius:8px;text-decoration:none;cursor:pointer}\
.choice{display:flex;gap:1rem;align-items:center}.choice .button{flex:1}.quiet{color:var(--soft)}\
.aside{margin:1.25rem 0 0;font-size:.9rem;text-align:center}.hint{color:var(--soft);font-size:.8rem;margin-top:1rem}\
.error{color:#c0392b;font-size:.9rem}.notice{color:#1e8449;font-size:.9rem}a{color:inherit}\
.mail{border:1px dashed var(--line);border-radius:8px;padding:1rem;margin-bottom:1rem}.from{color:var(--soft);font-size:.85rem;margin-top:0}\
ul{padding-left:1.2rem}";
