// Photographs the eight views of maquettes/macro-externe/apercu.html (the validated preview of an
// external macro's line, D-37) at 200 %, to set them beside the real HUD in
// maquettes/macro-externe/resultat.html (P4.5): apercu-1@2x.png … apercu-8@2x.png, next to the page.
//
// Usage: node maquettes/outils/apercu-externe.mjs

import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const HERE = dirname(fileURLToPath(import.meta.url));
const DIR = join(HERE, '..', 'macro-externe');

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1200, height: 900 }, deviceScaleFactor: 2 });
await page.goto(pathToFileURL(join(DIR, 'apercu.html')).href);
await page.evaluate(() => document.fonts.ready);
// The views are in the second section (« Avec une macro externe »), in order.
const tiles = await page.locator('section:nth-of-type(2) figure .tuile').all();
if (tiles.length !== 8) throw new Error(`8 vues attendues, ${tiles.length} trouvées`);
for (const [k, tile] of tiles.entries()) {
  const path = join(DIR, `apercu-${k + 1}@2x.png`);
  await tile.screenshot({ path });
  console.log(path);
}
await browser.close();
