// Renders maquettes/index.html to variante-1.png … variante-5.png (1600 px wide,
// real size at 100 % Windows scaling) and checks the text contrast of every HUD.
//
// Usage: node maquettes/outils/rendre.mjs [--gris <dossier>]
//   --gris <dossier>  also writes grayscale copies there (check that no state relies on colour)

import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '..');
const grayIdx = process.argv.indexOf('--gris');
const GRAY_DIR = grayIdx > 0 ? resolve(process.argv[grayIdx + 1]) : null;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 1200 }, deviceScaleFactor: 1 });
await page.goto(pathToFileURL(join(ROOT, 'index.html')).href);
await page.evaluate(() => document.fonts.ready);

const fontOk = await page.evaluate(() => document.fonts.check('700 14px Inter'));
if (!fontOk) throw new Error('police Inter non chargée');

async function shoot(dir, suffix = '') {
  for (const n of [1, 2, 3, 4, 5]) {
    const box = await page.locator(`#v${n}`).boundingBox();
    const path = join(dir, `variante-${n}${suffix}.png`);
    await page.screenshot({ path, fullPage: true, clip: { x: 0, y: Math.max(0, box.y - 20), width: 1600, height: Math.ceil(box.height) + 40 } });
    console.log(`image : ${path} (${Math.ceil(box.height) + 40} px de haut)`);
  }
}

await shoot(ROOT);

// Contrast check: every text node inside a HUD against its first opaque background.
const report = await page.evaluate(() => {
  const parse = (c) => {
    const m = c.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(',').map((x) => parseFloat(x));
    return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
  };
  const lin = (v) => {
    v /= 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const lum = (c) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
  const ratio = (a, b) => {
    const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
    return (x + 0.05) / (y + 0.05);
  };
  const out = {};
  for (const hud of document.querySelectorAll('.hud')) {
    const variant = hud.closest('.variant').id;
    const walker = document.createTreeWalker(hud, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const text = walker.currentNode.textContent.trim();
      if (!text) continue;
      const el = walker.currentNode.parentElement;
      const fg = parse(getComputedStyle(el).color);
      let bg = null;
      for (let e = el; e && e !== hud.parentElement; e = e.parentElement) {
        const c = parse(getComputedStyle(e).backgroundColor);
        if (c && c.a === 1) { bg = c; break; }
      }
      if (!bg) continue;
      const r = ratio(fg, bg);
      const disabled = !!el.closest('.soon, .off');
      const rec = (out[variant] ??= { min: 99, minText: '', minOff: 99, fails: [] });
      if (disabled) rec.minOff = Math.min(rec.minOff, r);
      else if (r < rec.min) { rec.min = r; rec.minText = text; }
      if (r < (disabled ? 3 : 4.5)) rec.fails.push(`${text} (${r.toFixed(2)})`);
    }
  }
  return out;
});

let failures = 0;
for (const [v, rec] of Object.entries(report)) {
  const off = rec.minOff < 99 ? `, texte grisé ${rec.minOff.toFixed(2)}:1` : '';
  console.log(`${v} : contraste minimal ${rec.min.toFixed(2)}:1 (« ${rec.minText} »)${off}${rec.fails.length ? ' ; ÉCHECS : ' + [...new Set(rec.fails)].join(', ') : ''}`);
  failures += rec.fails.length;
}

// Layout check: no text cut or pushed out of its box, nothing outside the HUD panel.
const overflows = await page.evaluate(() => {
  const found = [];
  for (const hud of document.querySelectorAll('.hud')) {
    const variant = hud.closest('.variant').id;
    const box = hud.getBoundingClientRect();
    for (const el of hud.querySelectorAll('*')) {
      if (el instanceof SVGElement && el.closest('svg') !== el) continue;
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      const label = (el.textContent || el.getAttribute('class') || el.tagName).trim().slice(0, 40);
      if (r.left < box.left - 0.5 || r.right > box.right + 0.5) found.push(`${variant} : sort du HUD : ${label}`);
      if (!(el instanceof SVGElement) && el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow === 'visible') {
        found.push(`${variant} : texte trop large (${el.scrollWidth} > ${el.clientWidth}) : ${label}`);
      }
      if (!(el instanceof SVGElement) && el.scrollHeight > el.clientHeight + 1 && getComputedStyle(el).overflow === 'visible') {
        found.push(`${variant} : texte trop haut (${el.scrollHeight} > ${el.clientHeight}) : ${label}`);
      }
    }
  }
  return [...new Set(found)];
});
for (const o of overflows) console.log(o);
failures += overflows.length;

if (GRAY_DIR) {
  await page.addStyleTag({ content: 'html{filter:grayscale(1)}' });
  await shoot(GRAY_DIR, '-gris');
}

await browser.close();
if (failures) {
  console.error(`${failures} problème(s) de contraste ou de mise en page`);
  process.exit(1);
}
