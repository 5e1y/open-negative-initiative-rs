//! The simulated pages' one stylesheet: The Lens DB's zinc ink on white, inverted in the dark, ONI's orange
//! only where a page speaks of ONI; readable from 360 pixels wide.

pub const STYLE: &str = r#"
:root{color-scheme:light dark;--ground:#fafafa;--card:#fff;--ink:#18181b;--soft:#71717a;--faint:#a1a1aa;
--line:#e4e4e7;--well:#f4f4f5;--field:#fff;--disc:#09090b;--blades:#fff;--go:#18181b;--on-go:#fafafa;
--ring:rgba(24,24,27,.18);--yes:#15803d;--yes-well:#dcfce7;--no:#71717a;--no-well:#f4f4f5;--oni:#f26a0c;
--sim:#92400e;--sim-well:#fef3c7;--error:#b91c1c;--error-well:#fef2f2;--fine:#15803d;--fine-well:#f0fdf4;
--shadow:0 1px 2px rgba(9,9,11,.04),0 12px 32px -12px rgba(9,9,11,.12)}
@media (prefers-color-scheme:dark){:root{--ground:#09090b;--card:#111113;--ink:#fafafa;--soft:#a1a1aa;--faint:#71717a;
--line:#27272a;--well:#18181b;--field:#09090b;--disc:#fafafa;--blades:#09090b;--go:#fafafa;--on-go:#09090b;
--ring:rgba(250,250,250,.22);--yes:#4ade80;--yes-well:rgba(74,222,128,.12);--no:#a1a1aa;--no-well:#1f1f23;
--sim:#fcd34d;--sim-well:rgba(252,211,77,.12);--error:#fca5a5;--error-well:rgba(248,113,113,.1);--fine:#86efac;
--fine-well:rgba(74,222,128,.1);--shadow:0 1px 2px rgba(0,0,0,.4),0 12px 32px -12px rgba(0,0,0,.6)}}
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;min-height:100vh;background:var(--ground);color:var(--ink);-webkit-font-smoothing:antialiased;
font:15px/1.5 "Geist","Inter",ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
a{color:inherit}
.page{display:flex;flex-direction:column;align-items:center;min-height:100vh;padding:28px 16px 32px}
.top{display:flex;align-items:center;gap:10px;margin-bottom:28px}
.brand{display:flex;align-items:center;gap:10px;text-decoration:none;color:var(--ink)}
.brand span{font-weight:800;font-size:17px;letter-spacing:-.03em;line-height:1}
.sim{display:inline-flex;align-items:center;gap:6px;padding:3px 9px 3px 8px;border-radius:999px;font-size:11.5px;
font-weight:600;letter-spacing:.01em;color:var(--sim);background:var(--sim-well);cursor:help}
.sim::before{content:"";width:6px;height:6px;border-radius:50%;background:currentColor}
.card{width:100%;max-width:400px;background:var(--card);border:1px solid var(--line);border-radius:16px;
padding:32px;box-shadow:var(--shadow)}
.note{max-width:400px;margin:20px 0 0;color:var(--faint);font-size:12px;text-align:center;line-height:1.5;text-wrap:balance}
.note b{color:var(--soft);font-weight:600}
h1{font-size:22px;line-height:1.25;letter-spacing:-.02em;font-weight:700;margin:0 0 6px;text-wrap:balance}
h2{font-size:12px;font-weight:600;color:var(--soft);margin:0 0 10px}
.head{text-align:center}
.lead{color:var(--soft);margin:0 0 24px;text-wrap:balance}
.lead b{color:var(--ink);font-weight:600}
.client{display:block;width:56px;height:56px;margin:0 auto 14px}
.steps{display:flex;align-items:center;justify-content:center;gap:10px;list-style:none;padding:0;margin:0 0 26px;
font-size:12.5px;font-weight:500;color:var(--faint)}
.steps li{display:flex;align-items:center;gap:7px;white-space:nowrap}
.steps li+li::before{content:"";width:28px;height:1px;background:var(--line);margin-right:3px}
.steps i{display:grid;place-items:center;width:20px;height:20px;border-radius:50%;font-style:normal;font-size:11px;
font-weight:600;border:1px solid var(--line);color:var(--soft)}
.steps .now{color:var(--ink)}.steps .now i{background:var(--go);border-color:var(--go);color:var(--on-go)}
.steps .done{color:var(--soft)}.steps .done i{background:var(--well);color:var(--ink)}
.steps svg{width:11px;height:11px}
svg.glyph{fill:none;stroke:currentColor;stroke-width:2.4;stroke-linecap:round;stroke-linejoin:round}
.quick{display:flex;align-items:center;gap:12px;padding:12px 14px;border:1px solid var(--line);border-radius:12px;
text-decoration:none;background:var(--card);transition:border-color .15s,background .15s}
.quick:hover{border-color:var(--faint);background:var(--well)}
.quick .who{flex:1;min-width:0}.quick b{display:block;font-weight:600}
.quick small{display:block;color:var(--soft);font-size:12.5px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.quick>svg{width:16px;height:16px;color:var(--faint);flex:none}
.avatar{display:grid;place-items:center;flex:none;width:34px;height:34px;border-radius:50%;font-weight:600;
font-size:14px;color:#fff;background:linear-gradient(135deg,#52525b,#18181b)}
.or{display:flex;gap:12px;align-items:center;color:var(--faint);font-size:12px;margin:22px 0}
.or::before,.or::after{content:"";flex:1;border-top:1px solid var(--line)}
form{margin:0}
label{display:block;font-size:13px;font-weight:500;margin-bottom:14px}
input{display:block;width:100%;height:40px;margin-top:6px;padding:0 12px;font:inherit;color:var(--ink);
background:var(--field);border:1px solid var(--line);border-radius:8px;outline:none;transition:border-color .15s,box-shadow .15s}
input:focus{border-color:var(--soft);box-shadow:0 0 0 3px var(--ring)}
button,.primary,.secondary{display:flex;align-items:center;justify-content:center;width:100%;height:42px;padding:0 16px;
font:inherit;font-weight:600;border-radius:9px;text-decoration:none;cursor:pointer;transition:opacity .15s,background .15s}
button,.primary{color:var(--on-go);background:var(--go);border:1px solid var(--go)}
button:hover,.primary:hover{opacity:.88}
.secondary{color:var(--ink);background:var(--card);border:1px solid var(--line)}
.secondary:hover{background:var(--well)}
form button{margin-top:6px}
:focus-visible{outline:2px solid var(--ink);outline-offset:2px}
.aside{margin:20px 0 0;font-size:13.5px;text-align:center;color:var(--soft)}
.aside a{color:var(--ink);font-weight:600;text-decoration:none}.aside a:hover{text-decoration:underline}
.hint{color:var(--faint);font-size:12px;margin:14px 0 0;text-align:center;text-wrap:balance}
.hint b{color:var(--soft);font-weight:600}
.error,.notice{font-size:13px;padding:10px 12px;border-radius:8px;margin:0 0 16px}
.error{color:var(--error);background:var(--error-well)}.notice{color:var(--fine);background:var(--fine-well)}
.badge{display:grid;place-items:center;width:52px;height:52px;margin:0 auto 16px;border-radius:50%;
background:var(--well);color:var(--ink)}.badge svg{width:22px;height:22px}
.mail{border:1px solid var(--line);border-radius:12px;overflow:hidden;margin:0 0 4px}
.mail dl{display:grid;grid-template-columns:auto 1fr;gap:4px 12px;margin:0;padding:12px 16px;font-size:12.5px;
background:var(--well);border-bottom:1px solid var(--line)}
.mail dt{color:var(--faint)}.mail dd{margin:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.mail dd.from{display:flex;align-items:center;gap:8px}.mail .sim{font-size:10.5px;padding:1px 8px 1px 7px}
.mail .body{padding:16px}.mail .body p{margin:0 0 14px}
.pair{display:flex;align-items:center;justify-content:center;margin:4px 0 24px}
.pair img{width:76px;height:76px;animation:in-left .5s cubic-bezier(.2,.8,.2,1) both}
.pair .tl{display:grid;place-items:center;width:76px;height:76px;animation:in-right .5s cubic-bezier(.2,.8,.2,1) both}
.pair .tl svg{border-radius:50%;box-shadow:0 6px 16px -6px rgba(9,9,11,.45)}
.link{display:flex;align-items:center;gap:6px;width:112px;padding:0 6px}
.link i{flex:1;height:2px;border-radius:2px;animation:draw .45s .25s ease-out both;
background:repeating-linear-gradient(90deg,var(--soft) 0 5px,transparent 5px 9px)}
.link i:first-child{transform-origin:left;background:repeating-linear-gradient(90deg,var(--oni) 0 5px,transparent 5px 9px)}
.link i:last-child{transform-origin:right}
.link b{display:grid;place-items:center;flex:none;width:36px;height:36px;border-radius:50%;background:var(--card);
border:1px solid var(--line);color:var(--ink);box-shadow:0 2px 6px -2px rgba(9,9,11,.12);animation:pop .4s .6s cubic-bezier(.3,1.6,.5,1) both}
.link svg{width:17px;height:17px;stroke-width:2.2}
@keyframes in-left{from{opacity:0;transform:translateX(14px) scale(.94)}}
@keyframes in-right{from{opacity:0;transform:translateX(-14px) scale(.94)}}
@keyframes draw{from{transform:scaleX(0)}}
@keyframes pop{from{opacity:0;transform:scale(.5)}}
.chip{display:flex;align-items:center;gap:10px;padding:6px 6px 6px 6px;margin:0 0 22px;border:1px solid var(--line);
border-radius:999px;font-size:13px}
.chip .avatar{width:26px;height:26px;font-size:12px}
.chip .id{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--soft)}
.chip .id b{color:var(--ink);font-weight:600}
.chip a{flex:none;padding:4px 10px;border-radius:999px;font-weight:500;text-decoration:none;color:var(--soft)}
.chip a:hover{background:var(--well);color:var(--ink)}
.scope{border:1px solid var(--line);border-radius:12px;padding:16px;margin:0 0 24px}
.scope ul{list-style:none;margin:0;padding:0}
.scope ul+h2{margin-top:16px;padding-top:16px;border-top:1px solid var(--line)}
.scope li{display:flex;align-items:center;gap:11px;font-size:14px;margin:0 0 10px}.scope li:last-child{margin:0}
.scope li span{display:grid;place-items:center;flex:none;width:22px;height:22px;border-radius:50%}
.scope li svg{width:12px;height:12px}
.yes span{background:var(--yes-well);color:var(--yes)}
.no{color:var(--soft)}.no span{background:var(--no-well);color:var(--no)}
.choice{display:flex;gap:10px}.choice>*{flex:1}
.fine{margin:16px 0 0;font-size:12.5px;color:var(--faint);text-align:center}
.card ul.facts{padding-left:18px;color:var(--soft)}
@media (max-width:420px){.card{padding:24px 20px;border-radius:14px}.page{padding-top:20px}.top{margin-bottom:20px}}
@media (prefers-reduced-motion:reduce){*,*::before,*::after{animation:none!important;transition:none!important}}
"#;
