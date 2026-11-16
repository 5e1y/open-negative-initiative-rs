//! The sign-in's two steps in one card: first sign in (or create an account and confirm its e-mail), then the
//! consent that authorises ONI on the Kit; the step ahead stays folded in view, the step done folds to a row.

use super::super::accounts::{Account, FIRST_EMAIL, FIRST_NAME, FIRST_PASSWORD};
use super::super::http::{Response, escape};
use super::{Form, avatar, error, flow, glyph, hidden, notice, open};

/// The one-click way in for a check by hand: the first account signed in, straight to the consent.
fn quick(oauth: &str) -> String {
    format!(
        "<a class=\"quick\" id=\"quick\" href=\"{}\">{}<span class=\"who\"><b>Continue as {FIRST_NAME}</b>\
         <small>Test account · {FIRST_EMAIL}</small></span>{}</a><p class=\"or\">or</p>",
        escape(&format!("/oauth/quick?{oauth}")),
        avatar(FIRST_NAME),
        glyph("next")
    )
}

/// The consent still ahead, folded: what it will ask, so the end of the sign-in shows from its start.
fn ahead() -> String {
    "<li class=\"step ahead\"><header><i>2</i><h2>Authorize ONI</h2></header>\
     <p>ONI will ask to read and edit your Kit.</p></li>"
        .into()
}

/// The sign-in done, folded: who signed in, and `other`, the way to sign in as someone else.
fn done(account: &Account, other: &str) -> String {
    format!(
        "<li class=\"step done\"><header><i>{}</i><span class=\"id\">Signed in as <b>{}</b><small>{}</small></span>\
         <a href=\"{}\">Not you?</a></header></li>",
        glyph("check"),
        escape(&account.name),
        escape(&account.email),
        escape(other)
    )
}

/// The authorisation address again, on the sign-in page or the sign-up one.
pub fn authorize_link(oauth: &str, signup: bool) -> String {
    let plain = oauth.replace("&screen_hint=signup", "");
    if signup { format!("/oauth/authorize?{plain}&screen_hint=signup") } else { format!("/oauth/authorize?{plain}") }
}

pub fn sign_in(oauth: &str, form: &Form) -> Response {
    let content = format!(
        "{}{}{}<form method=\"post\" action=\"/oauth/sign-in\">{}\
         <label>E-mail<input name=\"email\" type=\"email\" autocomplete=\"off\" value=\"{}\" required autofocus></label>\
         <label>Password<input name=\"password\" type=\"password\" required></label>\
         <button>Sign in</button></form>\
         <p class=\"aside\">No account yet? <a href=\"{}\">Create one</a></p>\
         <p class=\"hint\">Test account password: <b>{FIRST_PASSWORD}</b></p>",
        notice(form),
        quick(oauth),
        error(form),
        hidden(oauth),
        escape(&form.email),
        escape(&authorize_link(oauth, true))
    );
    Response::html(200, flow("Sign in", &open(1, "Sign in", &content), &ahead()))
}

pub fn sign_up(oauth: &str, form: &Form) -> Response {
    let content = format!(
        "{}{}{}<form method=\"post\" action=\"/oauth/sign-up\">{}\
         <label>Name<input name=\"name\" autocomplete=\"off\" value=\"{}\" required autofocus></label>\
         <label>E-mail<input name=\"email\" type=\"email\" autocomplete=\"off\" value=\"{}\" required></label>\
         <label>Password<input name=\"password\" type=\"password\" required></label>\
         <button>Create account</button></form>\
         <p class=\"hint\">Use a throwaway password: it is stored in plain text on this computer.</p>\
         <p class=\"aside\">Have an account? <a href=\"{}\">Sign in</a></p>",
        notice(form),
        quick(oauth),
        error(form),
        hidden(oauth),
        escape(&form.name),
        escape(&form.email),
        escape(&authorize_link(oauth, false))
    );
    Response::html(200, flow("Create your account", &open(1, "Create your account", &content), &ahead()))
}

/// After a sign-up, or a sign-in before the confirmation: the e-mail the real site would send, shown here.
pub fn mailbox(email: &str, link: &str, again: bool) -> Response {
    let lead = if again { "Confirm your e-mail before signing in." } else { "Account created. Confirm your e-mail to sign in." };
    let content = format!(
        "<p class=\"lead\">{lead}</p>\
         <div class=\"mail\"><dl><dt>From</dt><dd class=\"from\">The Lens DB<span class=\"sim\">Simulated e-mail</span></dd>\
         <dt>To</dt><dd>{}</dd><dt>Subject</dt><dd>Confirm your e-mail</dd></dl>\
         <div class=\"body\"><p>Confirm this address to finish creating your account.</p>\
         <a class=\"primary\" id=\"confirm\" href=\"{}\">Confirm my e-mail</a></div></div>\
         <p class=\"hint\">ONI waits 15 minutes for you to finish.</p>",
        escape(email),
        escape(link)
    );
    Response::html(200, flow("Check your e-mail", &open(1, "Check your e-mail", &content), &ahead()))
}

/// The second step: the sign-in folded above, what ONI may do and never sees, and the person's answer.
pub fn consent(account: &Account, allow: &str, refuse: &str, other: &str) -> Response {
    let may = |text: &str| format!("<li class=\"yes\"><span>{}</span>{text}</li>", glyph("check"));
    let never = |text: &str| format!("<li class=\"no\"><span>{}</span>{text}</li>", glyph("cross"));
    let content = format!(
        "<p class=\"lead\">ONI keeps a copy of your Kit to fill in the camera and lens of your exports.</p>\
         <section class=\"scope\"><h2>ONI can</h2><ul>{}{}</ul><h2>ONI never sees</h2><ul>{}{}{}</ul></section>\
         <div class=\"choice\"><a class=\"secondary\" id=\"refuse\" href=\"{}\">Cancel</a>\
         <a class=\"primary\" id=\"allow\" href=\"{}\">Authorize ONI</a></div>\
         <p class=\"fine\">You can disconnect at any time from ONI.</p>",
        may("See the bodies and lenses in your Kit"),
        may("Add pieces and remove them"),
        never("Your notes"),
        never("What you paid"),
        never("Serial numbers"),
        escape(refuse),
        escape(allow)
    );
    Response::html(200, flow("Authorize ONI", &done(account, other), &open(2, "Authorize ONI", &content)))
}
