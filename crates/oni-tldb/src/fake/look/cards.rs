//! The sign-in's two steps as cards: first sign in (or create an account and confirm its e-mail), then the
//! consent that authorises ONI on the Kit, its two marks joined.

use super::super::accounts::{Account, FIRST_EMAIL, FIRST_NAME, FIRST_PASSWORD};
use super::super::http::{Response, escape};
use super::{Form, avatar, client, error, glyph, hidden, notice, shell, steps};
use crate::marks;

/// The one-click way in for a check by hand: the first account signed in, straight to the consent.
fn quick(oauth: &str) -> String {
    format!(
        "<a class=\"quick\" id=\"quick\" href=\"{}\">{}<span class=\"who\"><b>Continue with the test account</b>\
         <small>{FIRST_NAME} · {FIRST_EMAIL}</small></span>{}</a><p class=\"or\">or</p>",
        escape(&format!("/oauth/quick?{oauth}")),
        avatar(FIRST_NAME),
        glyph("next")
    )
}

/// The authorisation address again, on the sign-in page or the sign-up one.
pub fn authorize_link(oauth: &str, signup: bool) -> String {
    let plain = oauth.replace("&screen_hint=signup", "");
    if signup { format!("/oauth/authorize?{plain}&screen_hint=signup") } else { format!("/oauth/authorize?{plain}") }
}

pub fn sign_in(oauth: &str, form: &Form) -> Response {
    let body = format!(
        "{}<div class=\"head\">{}<h1>Sign in</h1><p class=\"lead\">to continue to <b>ONI</b></p></div>{}{}{}\
         <form method=\"post\" action=\"/oauth/sign-in\">{}\
         <label>E-mail<input name=\"email\" type=\"email\" autocomplete=\"off\" value=\"{}\" required autofocus></label>\
         <label>Password<input name=\"password\" type=\"password\" required></label>\
         <button>Sign in</button></form>\
         <p class=\"aside\">No account yet? <a href=\"{}\">Create one</a></p>\
         <p class=\"hint\">Test account <b>{FIRST_EMAIL}</b>, password <b>{FIRST_PASSWORD}</b></p>",
        steps("Sign in", 1),
        client(),
        notice(form),
        quick(oauth),
        error(form),
        hidden(oauth),
        escape(&form.email),
        escape(&authorize_link(oauth, true))
    );
    Response::html(200, shell("Sign in", &body))
}

pub fn sign_up(oauth: &str, form: &Form) -> Response {
    let body = format!(
        "{}<div class=\"head\">{}<h1>Create your account</h1><p class=\"lead\">to continue to <b>ONI</b></p></div>{}{}{}\
         <form method=\"post\" action=\"/oauth/sign-up\">{}\
         <label>Name<input name=\"name\" autocomplete=\"off\" value=\"{}\" required autofocus></label>\
         <label>E-mail<input name=\"email\" type=\"email\" autocomplete=\"off\" value=\"{}\" required></label>\
         <label>Password<input name=\"password\" type=\"password\" required></label>\
         <button>Create the account</button></form>\
         <p class=\"hint\">Invent a password: this simulation keeps it in plain text on this computer.</p>\
         <p class=\"aside\">Already registered? <a href=\"{}\">Sign in</a></p>",
        steps("Create account", 1),
        client(),
        notice(form),
        quick(oauth),
        error(form),
        hidden(oauth),
        escape(&form.name),
        escape(&form.email),
        escape(&authorize_link(oauth, false))
    );
    Response::html(200, shell("Create your account", &body))
}

/// After a sign-up, or a sign-in before the confirmation: the e-mail the real site would send, shown here.
pub fn mailbox(email: &str, link: &str, again: bool) -> Response {
    let lead = if again { "Confirm your e-mail before signing in." } else { "Your account is created. Confirm your e-mail to sign in." };
    let body = format!(
        "{}<div class=\"head\"><div class=\"badge\">{}</div><h1>Check your e-mail</h1><p class=\"lead\">{lead}</p></div>\
         <div class=\"mail\"><dl><dt>From</dt><dd class=\"from\">The Lens DB<span class=\"sim\">Simulated e-mail</span></dd>\
         <dt>To</dt><dd>{}</dd><dt>Subject</dt><dd>Confirm your e-mail</dd></dl>\
         <div class=\"body\"><p>Welcome to The Lens DB. Confirm this address to use your account.</p>\
         <a class=\"primary\" id=\"confirm\" href=\"{}\">Confirm my e-mail</a></div></div>\
         <p class=\"hint\">ONI waits up to 15 minutes for this sign-in to end.</p>",
        steps(if again { "Sign in" } else { "Create account" }, 1),
        glyph("mail"),
        escape(email),
        escape(link)
    );
    Response::html(200, shell("Check your e-mail", &body))
}

/// The second step: ONI and The Lens DB joined, what ONI may do and never sees, and the person's answer.
pub fn consent(account: &Account, allow: &str, refuse: &str, other: &str) -> Response {
    let may = |text: &str| format!("<li class=\"yes\"><span>{}</span>{text}</li>", glyph("check"));
    let never = |text: &str| format!("<li class=\"no\"><span>{}</span>{text}</li>", glyph("cross"));
    let body = format!(
        "{}<div class=\"pair\" aria-hidden=\"true\"><img src=\"{}\" width=\"76\" height=\"76\" alt=\"\">\
         <span class=\"link\"><i></i><b>{}</b><i></i></span><span class=\"tl\">{}</span></div>\
         <div class=\"head\"><h1>ONI wants to connect to your Kit</h1>\
         <p class=\"lead\"><b>ONI</b>, the film negative converter, asks for access to your Kit on The Lens DB.</p></div>\
         <div class=\"chip\">{}<span class=\"id\"><b>{}</b> · {}</span><a href=\"{}\">Not you?</a></div>\
         <section class=\"scope\"><h2>ONI will be able to</h2><ul>{}{}</ul><h2>ONI never sees</h2><ul>{}{}{}</ul></section>\
         <div class=\"choice\"><a class=\"secondary\" id=\"refuse\" href=\"{}\">Cancel</a>\
         <a class=\"primary\" id=\"allow\" href=\"{}\">Authorize ONI</a></div>\
         <p class=\"fine\">You can disconnect ONI at any time, from ONI.</p>",
        steps("Sign in", 2),
        marks::oni(),
        glyph("link"),
        marks::tldb(62),
        avatar(&account.name),
        escape(&account.name),
        escape(&account.email),
        escape(other),
        may("Read your bodies and lenses"),
        may("Add and remove pieces"),
        never("Your notes"),
        never("What you paid"),
        never("Serial numbers"),
        escape(refuse),
        escape(allow)
    );
    Response::html(200, shell("Authorize ONI", &body))
}
