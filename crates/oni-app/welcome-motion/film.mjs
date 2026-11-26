// Films one composition frame by frame in headless Chrome: `node film.mjs <name> <frames dir> name=dir...`, each
// `name=dir` served at `/name/`; the page's timeline is sought at each frame's time and each frame kept as a PNG.
// Chrome is the system's (`CHROME` names another): no browser is downloaded.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const FPS = 25;
const [name, out, ...mounts] = process.argv.slice(2);
const here = path.dirname(new URL(import.meta.url).pathname);
const roots = Object.fromEntries([['', here], ['gsap', path.join(here, 'node_modules/gsap/dist')], ...mounts.map((m) => m.split('='))]);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.jpg': 'image/jpeg', '.png': 'image/png', '.svg': 'image/svg+xml', '.ttf': 'font/ttf' };
// Served over HTTP: Chrome refuses fonts read from a file:// page.
const server = http.createServer((req, res) => {
  const [, first, ...rest] = decodeURIComponent(req.url.split('?')[0]).split('/');
  const file = first in roots && first !== '' ? path.join(roots[first], ...rest) : path.join(here, first, ...rest);
  fs.readFile(file, (err, data) => {
    res.writeHead(err ? 404 : 200, { 'content-type': types[path.extname(file)] || 'application/octet-stream' });
    res.end(err ? undefined : data);
  });
}).listen(0, '127.0.0.1');
await new Promise((r) => server.once('listening', r));
const chrome = process.env.CHROME || (process.platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : 'C:/Program Files/Google/Chrome/Application/chrome.exe');
const browser = await puppeteer.launch({ executablePath: chrome, headless: true, protocolTimeout: 120000, args: ['--force-color-profile=srgb', '--hide-scrollbars'] });
const page = await browser.newPage();
page.on('console', (m) => m.type() === 'error' && console.error(`${name}: ${m.text()}`));
// `SCALE` films each frame at that many pixels a point, for a screen of more pixels.
await page.setViewport({ width: 800, height: 500, deviceScaleFactor: Number(process.env.SCALE || 1) });
await page.goto(`http://127.0.0.1:${server.address().port}/${name}.html`, { waitUntil: 'load' });
const fonts = await page.evaluate(async () => { await document.fonts.ready; return [...document.fonts].filter((f) => f.status !== 'loaded').length; });
if (fonts) throw new Error(`${name}: ${fonts} fonts did not load`);
await page.waitForFunction(() => window.READY, { timeout: 60000 });
await page.evaluate(async () => { await window.READY; });
const length = await page.evaluate(() => window.LENGTH);
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
const count = Math.round(length * FPS);
for (let i = 0; i < count; i++) {
  await page.evaluate(async (s) => { await window.seek(s); }, i / FPS);
  await page.screenshot({ path: path.join(out, String(i).padStart(4, '0') + '.png'), clip: { x: 0, y: 0, width: 800, height: 500 } });
}
await browser.close();
server.close();
console.log(`${name}: ${count} frames, ${length.toFixed(2)} s`);
