//! The page the browser shows once it returns to ONI's port: ONI's mark with the outcome, the way back to the
//! app, and ONI joined to The Lens DB; it stays open, never closing itself.

use oni_i18n::tr;
use oni_tldb::marks;

/// Text made safe inside HTML.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A line icon, drawn by the text's colour.
fn glyph(path: &str) -> String {
    format!("<svg viewBox=\"0 0 24 24\" aria-hidden=\"true\">{path}</svg>")
}

pub fn landing(signed_in: bool) -> String {
    let (title, words, state) = if signed_in {
        (tr("You’re connected"), tr("Your Kit from The Lens DB is now in ONI. You can close this tab and go back to ONI."), "ok")
    } else {
        (tr("ONI was not connected"), tr("Nothing was shared. You can close this tab and try again from ONI."), "no")
    };
    let (badge, join) = if signed_in {
        (glyph("<path d=\"M20 6 9 17l-5-5\"/>"), glyph("<path d=\"M9 17H7A5 5 0 0 1 7 7h2M15 7h2a5 5 0 1 1 0 10h-2M8 12h8\"/>"))
    } else {
        let cross = glyph("<path d=\"M18 6 6 18M6 6l12 12\"/>");
        (cross.clone(), cross)
    };
    format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} · ONI</title><style>:root{{--oni:url({})}}{STYLE}</style></head>\
         <body><main class=\"{state}\"><div class=\"mark\"><span class=\"oni\" role=\"img\" aria-label=\"ONI\"></span>\
         <span class=\"badge\">{badge}</span></div><h1>{}</h1><p>{}</p>\
         <div class=\"pair\"><span><span class=\"oni\"></span>ONI</span><i>{join}</i>\
         <span>{}<b>THE LENS DB</b></span></div></main></body></html>",
        oni_i18n::current().key(),
        escape(title),
        marks::oni(),
        escape(title),
        escape(words),
        marks::tldb(20)
    )
}

const STYLE: &str = r#"
:root{color-scheme:light dark;--ground:#f5f5f4;--ink:#1c1917;--soft:#78716c;--line:#e7e5e4;--pill:#fff;--disc:#0c0a09;
--blades:#fff;--ok:#28c350;--no:#8e8e93;--ring:#f5f5f4}
@media (prefers-color-scheme:dark){:root{--ground:#141414;--ink:#f5f5f4;--soft:#a8a29e;--line:#2a2a2a;--pill:#1c1c1c;
--disc:#f5f5f4;--blades:#141414;--ok:#30d158;--no:#636366;--ring:#141414}}
*{box-sizing:border-box}
body{margin:0;min-height:100vh;display:grid;place-items:center;padding:32px 16px;background:var(--ground);color:var(--ink);
font:16px/1.55 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;-webkit-font-smoothing:antialiased}
main{max-width:420px;text-align:center;margin-top:-6vh;animation:fade .2s ease-out both}
.mark{position:relative;width:128px;height:128px;margin:0 auto 26px}
.oni{display:block;width:22px;height:22px;background:var(--oni) center/contain no-repeat}
.mark .oni{width:128px;height:128px;filter:drop-shadow(0 14px 22px rgba(242,106,12,.28))}
.no .mark .oni{filter:drop-shadow(0 10px 18px rgba(0,0,0,.14))}
.badge{position:absolute;right:2px;bottom:4px;display:grid;place-items:center;width:42px;height:42px;border-radius:50%;
color:#fff;background:var(--ok);border:4px solid var(--ring)}
.no .badge{background:var(--no)}
svg{fill:none;stroke:currentColor;stroke-width:3;stroke-linecap:round;stroke-linejoin:round}
.badge svg{width:20px;height:20px}
h1{font-size:28px;line-height:1.2;letter-spacing:-.025em;font-weight:700;margin:0 0 10px;text-wrap:balance}
p{color:var(--soft);margin:0 auto 32px;max-width:340px;text-wrap:balance}
.pair{display:inline-flex;align-items:center;gap:12px;padding:8px 16px 8px 10px;border:1px solid var(--line);
border-radius:999px;background:var(--pill);font-size:12.5px;font-weight:600;color:var(--soft)}
.pair span{display:flex;align-items:center;gap:8px}
.pair b{font-weight:800;letter-spacing:-.03em;color:var(--ink)}
.pair span:first-child{color:var(--ink);letter-spacing:.02em}
.pair i{display:grid;place-items:center;color:var(--soft)}.pair i svg{width:14px;height:14px;stroke-width:2.4}
.ok .pair i{color:var(--ok)}
@keyframes fade{from{opacity:0}}
@media (prefers-reduced-motion:reduce){*{animation:none!important}}
"#;

#[cfg(test)]
mod tests {
    use super::landing;

    #[test]
    fn the_page_says_the_outcome_with_both_marks() {
        let (yes, no) = (landing(true), landing(false));
        assert!(yes.contains("<main class=\"ok\">") && no.contains("<main class=\"no\">"));
        assert!(yes.contains("data:image/png;base64,") && yes.contains("THE LENS DB") && yes.contains("--disc"));
        // Twin: a refusal never shows the success's mark.
        assert!(!no.contains("M20 6 9 17l-5-5"), "{no}");
    }
}
