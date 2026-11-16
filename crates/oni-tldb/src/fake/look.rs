//! The simulated pages' HTML: one shell under The Lens DB's mark and a "Simulated" pill, a note under every
//! card that says what they are, the two-step sign-in's cards (`cards`) and the catalogue's plain pages.

mod cards;
mod style;

pub use cards::{authorize_link, consent, mailbox, sign_in, sign_up};

use super::http::{Response, escape};
use crate::marks;
use crate::piece::Piece;

/// What a sign-in or sign-up form shows again: the typed values and a line above them.
#[derive(Default)]
pub struct Form {
    pub name: String,
    pub email: String,
    pub error: String,
    pub notice: String,
}

/// A line icon, drawn by the text's colour.
fn glyph(name: &str) -> String {
    let path = match name {
        "check" => "<path d=\"M20 6 9 17l-5-5\"/>",
        "cross" => "<path d=\"M18 6 6 18M6 6l12 12\"/>",
        "next" => "<path d=\"m9 6 6 6-6 6\"/>",
        "link" => "<path d=\"M9 17H7A5 5 0 0 1 7 7h2M15 7h2a5 5 0 1 1 0 10h-2M8 12h8\"/>",
        "mail" => "<rect x=\"3\" y=\"5\" width=\"18\" height=\"14\" rx=\"2\"/><path d=\"m3 7 9 6 9-6\"/>",
        _ => "<path d=\"M12 8v5M12 16.5v.01\"/><circle cx=\"12\" cy=\"12\" r=\"9\"/>",
    };
    format!("<svg class=\"glyph\" viewBox=\"0 0 24 24\" aria-hidden=\"true\">{path}</svg>")
}

/// A page of the two steps, the second always in view: ONI and The Lens DB joined over the card, then each
/// step's markup in it.
fn flow(title: &str, first: &str, second: &str) -> String {
    let above = format!(
        "<div class=\"intro\"><div class=\"pair\" aria-hidden=\"true\"><img src=\"{}\" width=\"44\" height=\"44\" alt=\"\">\
         <span class=\"link\">{}</span>{}</div><p class=\"lead\">Connect <b>ONI</b> to your Kit on The Lens DB</p></div>",
        marks::oni(),
        glyph("link"),
        marks::tldb(44)
    );
    page(title, &above, "card steps", &format!("<ol class=\"flow\">{first}{second}</ol>"))
}

/// The step at hand, unfolded: its number, its title as the page's heading, and its content.
fn open(n: u8, title: &str, content: &str) -> String {
    format!("<li class=\"step open\" aria-current=\"step\"><header><i>{n}</i><h1>{title}</h1></header><div class=\"fold\">{content}</div></li>")
}

/// A person's initial, in a round mark.
fn avatar(name: &str) -> String {
    let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    format!("<span class=\"avatar\" aria-hidden=\"true\">{}</span>", escape(&initial))
}

pub fn refused(reason: &str) -> Response {
    let body = format!(
        "<div class=\"head\"><div class=\"badge\">{}</div><h1>This request is refused</h1><p class=\"lead\">{}</p></div>",
        glyph("alert"),
        escape(reason)
    );
    Response::html(400, shell("Refused", &body))
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
    let body = format!("<h1>{}</h1><p class=\"lead\">{kind} {}</p><ul class=\"facts\">{list}</ul>", escape(&piece.name), piece.id);
    Response::html(200, shell(&piece.name, &body))
}

fn hidden(oauth: &str) -> String {
    format!("<input type=\"hidden\" name=\"oauth\" value=\"{}\">", escape(oauth))
}

/// The line over the test account's button: the confirmation's notice.
fn notice(form: &Form) -> String {
    if form.notice.is_empty() { String::new() } else { format!("<p class=\"notice\" role=\"status\">{}</p>", escape(&form.notice)) }
}

/// The line over the form's fields: what was wrong with them.
fn error(form: &Form) -> String {
    if form.error.is_empty() { String::new() } else { format!("<p class=\"error\" role=\"alert\">{}</p>", escape(&form.error)) }
}

/// Every page: The Lens DB's mark with the "Simulated" pill, one card, and the note that no account is real;
/// light or dark as the system is.
pub fn shell(title: &str, body: &str) -> String {
    page(title, "", "card", body)
}

/// `shell` with what stands `above` the card, and the card's own class.
fn page(title: &str, above: &str, class: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{} · The Lens DB (simulated)</title><style>{}</style></head><body><div class=\"page\">\
         <header class=\"top\"><a class=\"brand\" href=\"/\">{}<span>THE LENS DB</span></a>\
         <span class=\"sim\" title=\"A local stand-in, not thelensdb.com\">Simulated</span></header>\
         {above}<main class=\"{class}\">{body}</main>\
         <p class=\"note\"><b>Simulated.</b> A copy of The Lens DB running on this computer to develop ONI. \
         Not the real site, and no real account.</p></div></body></html>",
        escape(title),
        style::STYLE,
        marks::tldb(26)
    )
}
