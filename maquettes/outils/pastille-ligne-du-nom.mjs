// Photographs the HUDs of variant A of maquettes/macro-externe/pastille-ligne-du-nom.html (the step
// pill on the name line, validated: D-38, D-39) at 200 %, to set them beside the real HUD in
// maquettes/macro-externe/pastille-ligne-du-nom-resultat.html (P4.7): pastille-a-1@2x.png (the four
// pills) and pastille-a-2@2x.png (a long name), next to the page.
//
// Usage: node maquettes/outils/pastille-ligne-du-nom.mjs

import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const HERE = dirname(fileURLToPath(import.meta.url));
const DIR = join(HERE, '..', 'macro-externe');

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1200, height: 900 }, deviceScaleFactor: 2 });
await page.goto(pathToFileURL(join(DIR, 'pastille-ligne-du-nom.html')).href);
await page.evaluate(() => document.fonts.ready);
// The HUDs of the validated section, in order.
const huds = await page.locator('section.choix .hud').all();
if (huds.length !== 2) throw new Error(`2 HUD attendus dans la variante A, ${huds.length} trouvés`);
for (const [k, hud] of huds.entries()) {
  const path = join(DIR, `pastille-a-${k + 1}@2x.png`);
  await hud.screenshot({ path });
  console.log(path);
}
await browser.close();
