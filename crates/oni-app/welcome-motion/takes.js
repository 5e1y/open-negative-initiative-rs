// The films composed over the app's takes (`tests/window/welcome_films.rs`): a take is the whole window at twice
// its pixels, a JPEG a frame; a camera frames part of it, in the window's points, and a hand points and presses.
// A composition writes its timeline on `S` and `film(tl)` hands it to `film.mjs`.

export const W = 800, H = 500, SHAPE = H / W;
const FPS = 25;

export const S = { take: '', frame: 0, cam: { x: 0, y: 0, w: 1280 }, hand: { x: 1500, y: 1000 }, press: 0, ring: 0, veil: 0, fade: '', fadeFrame: 0, fadeShown: 0 };

const stage = document.createElement('div');
Object.assign(stage.style, { position: 'absolute', left: 0, top: 0, width: W + 'px', height: H + 'px', overflow: 'hidden', background: '#000' });
document.body.style.margin = '0';
document.body.appendChild(stage);
const shot = (z) => { const i = document.createElement('img'); Object.assign(i.style, { position: 'absolute', left: 0, top: 0, transformOrigin: '0 0', zIndex: z }); stage.appendChild(i); return i; };
const take = shot(1), fade = shot(2);
const veil = document.createElement('div');
Object.assign(veil.style, { position: 'absolute', inset: 0, background: '#000', zIndex: 3 });
stage.appendChild(veil);
const hand = document.createElement('div');
Object.assign(hand.style, { position: 'absolute', left: 0, top: 0, zIndex: 4 });
hand.innerHTML = '<div style="position:absolute;left:-16px;top:-16px;width:32px;height:32px;border-radius:50%;border:2px solid rgba(255,255,255,.92)"></div>'
  + '<svg viewBox="0 0 22 28" width="22" height="28" style="position:absolute;left:0;top:0;transform-origin:2px 2px"><path d="M2 2 L2 22 L7.5 17 L11.5 26 L15 24.5 L11 15.8 L18.5 15.8 Z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/></svg>';
stage.appendChild(hand);
const [ring, arrow] = hand.children;

export const takes = await (await fetch('/takes/takes.json')).json();
export const point = (name) => { const [x, y] = takes.points[name]; return { x, y }; };
/// A take's length, in seconds, at the films' pace.
export const length = (name) => (takes.takes[name] - 1) / FPS;
const src = (name, frame) => `/takes/${name}/${String(Math.max(0, Math.min(takes.takes[name] - 1, Math.round(frame)))).padStart(4, '0')}.jpg`;
/// A frame `w` points wide about a point, kept inside the window.
export const about = ({ x, y }, w) => ({ x: Math.max(0, Math.min(1280 - w, x - w / 2)), y: Math.max(0, Math.min(800 - w * SHAPE, y - (w * SHAPE) / 2)), w });

function place(img, name, frame) {
  const want = src(name, frame);
  if (!img.src.endsWith(want)) img.src = want;
  const scale = W / S.cam.w;
  Object.assign(img.style, { width: 1280 * scale + 'px', height: 800 * scale + 'px', transform: `translate(${-S.cam.x * scale}px, ${-S.cam.y * scale}px)` });
}

export function draw() {
  place(take, S.take, S.frame);
  fade.style.display = S.fade ? 'block' : 'none';
  if (S.fade) place(fade, S.fade, S.fadeFrame);
  fade.style.opacity = S.fadeShown;
  veil.style.opacity = S.veil;
  const scale = W / S.cam.w;
  hand.style.transform = `translate(${(S.hand.x - S.cam.x) * scale}px, ${(S.hand.y - S.cam.y) * scale}px)`;
  arrow.style.transform = `scale(${1 - 0.14 * S.press})`;
  ring.style.opacity = S.ring > 0 && S.ring < 1 ? 0.9 * (1 - S.ring) : 0;
  ring.style.transform = `scale(${0.4 + 1.2 * S.ring})`;
}

export const tl = gsap.timeline({ paused: true, onUpdate: draw, defaults: { ease: 'power3.inOut' } });

/// The take shown from `at`, held on its first frame.
export const cut = (name, at) => {
  if (at === 0) Object.assign(S, { take: name, frame: 0 });
  return tl.set(S, { take: name, frame: 0 }, at);
};
/// The take played from `at` at the films' pace; its length in seconds.
export function play(name, at) {
  cut(name, at);
  tl.to(S, { frame: takes.takes[name] - 1, duration: length(name), ease: 'none' }, at);
  return length(name);
}
/// The camera moved to `to` over `duration`.
export const look = (to, at, duration = 0.8, ease = 'power3.inOut') => tl.to(S.cam, { ...to, duration, ease }, at);
/// The hand moved to `to` over `duration`.
export const reach = (to, at, duration = 0.6) => tl.to(S.hand, { x: to.x, y: to.y, duration, ease: 'power3.inOut' }, at);
/// A press: the hand sinks a hair as its ring opens and fades.
export function press(at) {
  tl.to(S, { press: 1, duration: 0.08, ease: 'power1.out' }, at);
  tl.to(S, { press: 0, duration: 0.18, ease: 'power1.out' }, at + 0.08);
  tl.fromTo(S, { ring: 0 }, { ring: 1, duration: 0.45, ease: 'power2.out', immediateRender: false }, at);
}
/// The take `name` laid over the shown one from `at`, rising to whole over `duration`, then the only one.
export function dissolve(name, frame, at, duration) {
  tl.set(S, { fade: name, fadeFrame: frame, fadeShown: 0 }, at);
  tl.to(S, { fadeShown: 1, duration, ease: 'power1.inOut' }, at);
  tl.set(S, { take: name, frame, fade: '', fadeShown: 0 }, at + duration);
}

/// The composition handed to the filmer: its length, and a seek that waits for its pictures.
export function film(total) {
  const decoded = () => Promise.all([...document.images].filter((i) => i.src).map((i) => i.decode().catch(() => {})));
  window.LENGTH = total;
  window.seek = async (s) => { tl.seek(s, false); draw(); await decoded(); };
  tl.seek(0, false);
  draw();
  window.READY = decoded();
}
