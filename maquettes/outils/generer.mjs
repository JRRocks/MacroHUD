// Builds maquettes/index.html: five HUD mockup variants, each shown in six views (a-f).
// The page is self-contained: the Inter font (SIL OFL 1.1) is embedded, icons and
// background scenes are inline SVG, no external resource is loaded.
//
// Everything drawn inside a `.hud` element must stay drawable with egui: rounded
// rectangles, solid fills, strokes, one simple drop shadow, text, simple vector icons
// (circles, lines, convex polygons). Backgrounds behind the HUD are only scenery.
//
// Usage: node maquettes/outils/generer.mjs   (then rendre.mjs for the PNG files)

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, '..', 'index.html');
const FONT = readFileSync(join(HERE, 'polices', 'inter-latin-wght-normal.woff2')).toString('base64');

// ---------------------------------------------------------------------------
// Content, limited to docs/CONTRAT-HUD.md
// ---------------------------------------------------------------------------

const NBSP = ' ';
const VERSION = '0.1.7';
const OURS = { name: 'Maintenir le clic', desc: 'Garde un bouton de la souris enfoncé', key: 'F8' };
const BUTTONS = ['Gauche', 'Droit', 'Milieu'];
// The two "coming soon" macros are placeholders that only show how several macros fit.
const SOON = [
  { name: 'Clic répété', icon: 'repeat', key: 'F9', optLabel: 'Intervalle', optValue: '100 ms', summary: '100 ms' },
  { name: 'Touche maintenue', icon: 'key', key: 'F10', optLabel: 'Touche', optValue: 'Espace', summary: 'Espace' },
];
const CAPTURE = { t1: 'Appuie sur une touche…', t2: 'Échap pour annuler' };
const REFUSED = {
  t1: 'Raccourci refusé',
  t2: `Alt+Tab est déjà pris par Windows.<br>F8 reste actif.`,
  action: 'Choisir une autre touche',
};
const CLICK_HINT = 'Un clic gauche arrête aussi la macro.';

// The six views every variant shows.
const VIEWS = [
  { k: 'a', label: 'Arrêtée', bg: 'bureau', s: { state: 'stopped' } },
  { k: 'b', label: 'Active, bouton gauche enfoncé', bg: 'nuit', s: { state: 'active' } },
  { k: 'c', label: 'Capture d’un nouveau raccourci', bg: 'jour', s: { state: 'stopped', capture: true } },
  { k: 'd', label: 'Raccourci refusé', bg: 'bureau', s: { state: 'stopped', refused: true } },
  { k: 'e', label: 'Trois macros, dont deux « À venir »', bg: 'nuit', s: { state: 'stopped', multi: true } },
  { k: 'f', label: 'Repliée : arrêtée, puis active', bg: 'jour', s: { collapsed: true } },
];
const BG_NAMES = { bureau: 'fond : bureau clair', nuit: 'fond : jeu de nuit', jour: 'fond : jeu en plein jour' };

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

const ic = (name, cls = '') => `<svg class="ic ${cls}" viewBox="0 0 16 16" aria-hidden="true"><use href="#i-${name}"/></svg>`;
const kc = (text, cls = '') => `<span class="kc ${cls}">${text}</span>`;
const seg = (opts, sel) =>
  `<div class="seg">${opts.map((o) => (o === sel ? `<span class="sel">${ic('check')}${o}</span>` : `<span>${o}</span>`)).join('')}</div>`;
const capBox = (btn = 'sbtn') =>
  `<div class="cap"><div class="c1">${kc('…', 'wait')}<span class="t1">${CAPTURE.t1}</span></div>` +
  `<div class="c2"><span class="t2">${CAPTURE.t2}</span><span class="${btn}">Annuler</span></div></div>`;
// withAction = false where the « Changer » button sits right above the message.
const warnBox = (withAction = true) =>
  `<div class="msg">${ic('warn', 'wi')}<div><div class="t1">${REFUSED.t1}</div><div class="t2">${REFUSED.t2}</div>` +
  `${withAction ? `<span class="abtn">${REFUSED.action}</span>` : ''}</div></div>`;

// Mouse drawing (variant 4). Every part is a capsule, a line or a convex region,
// so egui can draw it with rect_filled / line_segment / convex_polygon.
function mouseFig({ sel = 'Gauche', on = false }) {
  const LEFT = 'M48 4V58H4V48A44 44 0 0 1 48 4Z';
  const RIGHT = 'M48 4V58H92V48A44 44 0 0 0 48 4Z';
  const fillFor = (b) => (b === sel ? (on ? '#34d27c' : '#3d4b66') : 'none');
  const marker = (cx) =>
    on
      ? `<path d="M${cx} 45 ${cx - 9} 34h5.5v-10h7v10h5.5z" fill="#04140a"/>`
      : `<circle cx="${cx}" cy="32" r="5" fill="#e3e9f2"/>`;
  let wheelFill = '#8a97ad';
  if (sel === 'Milieu') wheelFill = on ? '#34d27c' : '#e3e9f2';
  return `<svg class="fig" viewBox="0 0 96 132" width="96" height="132" aria-hidden="true">
    <rect x="4" y="4" width="88" height="124" rx="44" fill="#232b39"/>
    <path d="${LEFT}" fill="${fillFor('Gauche')}"/>
    <path d="${RIGHT}" fill="${fillFor('Droit')}"/>
    <rect x="4" y="4" width="88" height="124" rx="44" fill="none" stroke="#8a97ad" stroke-width="2"/>
    <path d="M4 58H92M48 4V58" stroke="#8a97ad" stroke-width="2"/>
    ${sel === 'Gauche' ? marker(28) : ''}${sel === 'Droit' ? marker(68) : ''}
    <rect x="42" y="14" width="12" height="28" rx="6" fill="${wheelFill}" stroke="#161b24" stroke-width="2"/>
  </svg>`;
}

// Small mouse pictogram with the held button filled.
function mouseMini(on) {
  return `<svg class="mm" viewBox="0 0 16 22" aria-hidden="true">
    <rect x="1" y="1" width="14" height="20" rx="7" fill="#232b39" stroke="#c4cdd9" stroke-width="1.5"/>
    <path d="M8 1V9H1V8A7 7 0 0 1 8 1Z" fill="${on ? '#34d27c' : '#9aa6b8'}"/>
    <path d="M1 9H15M8 1V9" stroke="#c4cdd9" stroke-width="1.5"/>
  </svg>`;
}

// ---------------------------------------------------------------------------
// Variant 1 — Cartes
// ---------------------------------------------------------------------------

function v1(s) {
  if (s.collapsed) return v1mini(false) + v1mini(true);
  let body = v1card(s);
  if (s.multi) body += SOON.map(v1soon).join('');
  return `<div class="hud v1 dark">
    <div class="bar">${ic('grip', 'grip')}<span class="brand">MacroHUD</span><span class="ver">${VERSION}</span><span class="sp"></span><span class="gbtn">${ic('up')}Replier</span></div>
    <div class="body">${body}</div>
    <div class="foot"><span class="tbtn">${ic('x')}Quitter</span></div>
  </div>`;
}

function v1card(s) {
  const on = s.state === 'active';
  const badge = on ? `<span class="badge on">${ic('dot')}ACTIVE</span>` : `<span class="badge">${ic('stop')}ARRÊTÉE</span>`;
  const desc = on ? `<div class="desc on">Bouton gauche enfoncé</div>` : `<div class="desc">${OURS.desc}</div>`;
  const key = s.capture
    ? capBox()
    : `<div class="row"><span class="lab">Raccourci</span><span class="sp"></span>${kc(OURS.key)}<span class="sbtn">Changer</span></div>`;
  const action = on
    ? `<div class="pbtn">${ic('stop')}Arrêter${kc(OURS.key)}</div><div class="hint">${CLICK_HINT}</div>`
    : `<div class="pbtn">${ic('play')}Activer${kc(OURS.key)}</div>`;
  return `<div class="card${on ? ' on' : ''}">
    <div class="head"><span class="name">${OURS.name}</span>${badge}</div>${desc}
    <div class="lab mt">Bouton maintenu</div>${seg(BUTTONS, 'Gauche')}
    ${key}${s.refused ? warnBox() : ''}${action}
  </div>`;
}

function v1soon(m) {
  return `<div class="card soon">
    <div class="head">${ic(m.icon, 'ti')}<span class="name">${m.name}</span><span class="badge soon">À VENIR</span></div>
    <div class="srow"><span>Raccourci</span>${kc(m.key, 'off')}<span>·</span><span>${m.optLabel}${NBSP}: ${m.optValue}</span></div>
  </div>`;
}

function v1mini(on) {
  const badge = on ? `<span class="badge on">${ic('dot')}ACTIVE</span>` : `<span class="badge">${ic('stop')}ARRÊTÉE</span>`;
  return `<div class="hud v1 dark mini${on ? ' on' : ''}">${ic('grip', 'grip')}${badge}<span class="name">${OURS.name}</span>${kc(OURS.key)}<span class="ibtn">${ic('down')}</span></div>`;
}

// ---------------------------------------------------------------------------
// Variant 2 — Liste compacte
// ---------------------------------------------------------------------------

function v2(s) {
  if (s.collapsed) return v2mini(false) + v2mini(true);
  const on = s.state === 'active';
  const open = !on && !s.multi;
  let rows = v2row(s, open);
  if (s.multi) rows += SOON.map(v2soon).join('');
  return `<div class="hud v2 dark">
    <div class="bar">${ic('grip', 'grip')}<span class="brand">MacroHUD</span><span class="ver">${VERSION}</span><span class="sp"></span><span class="tbtn">${ic('up')}Replier</span><span class="tbtn">${ic('x')}Quitter</span></div>
    ${rows}
  </div>`;
}

function v2row(s, open) {
  const on = s.state === 'active';
  const st = on ? `${ic('dot')}Active · gauche enfoncé` : `${ic('stop')}Arrêtée · bouton gauche`;
  const row = `<div class="row${on ? ' on' : ''}"><span class="stripe"></span><span class="tog">${ic(on ? 'stop' : 'play')}</span>
    <div class="txt"><div class="name">${OURS.name}</div><div class="st">${st}</div></div>${kc(OURS.key)}<span class="dis">${ic(open ? 'up' : 'down')}</span></div>`;
  if (!open) return row;
  const key = s.capture
    ? capBox()
    : `<div class="orow"><span class="lab">Raccourci</span><span class="sp"></span>${kc(OURS.key)}<span class="sbtn">Changer</span></div>`;
  return `${row}<div class="opts"><div class="lab">Bouton maintenu</div>${seg(BUTTONS, 'Gauche')}${key}${s.refused ? warnBox() : ''}</div>`;
}

function v2soon(m) {
  return `<div class="row soon"><span class="stripe"></span><span class="tico">${ic(m.icon)}</span>
    <div class="txt"><div class="name">${m.name}</div><div class="st">À venir · ${m.summary}</div></div>${kc(m.key, 'off')}<span class="dis off">${ic('down')}</span></div>`;
}

function v2mini(on) {
  return `<div class="hud v2 dark mini${on ? ' on' : ''}">${ic('grip', 'grip')}<span class="mst">${ic(on ? 'dot' : 'stop')}${on ? 'Active' : 'Arrêtée'}</span>${kc(OURS.key)}<span class="dis">${ic('down')}</span></div>`;
}

// ---------------------------------------------------------------------------
// Variant 3 — Grand état (light theme)
// ---------------------------------------------------------------------------

function v3(s) {
  if (s.collapsed) return v3mini(false) + v3mini(true);
  const on = s.state === 'active';
  const key = s.capture
    ? capBox()
    : `<div class="row"><span class="lab">Raccourci</span><span class="sp"></span>${kc(OURS.key)}<span class="sbtn">Changer</span></div>`;
  return `<div class="hud v3 light">
    <div class="bar">${ic('grip', 'grip')}<span class="brand">MacroHUD</span><span class="ver">${VERSION}</span><span class="sp"></span><span class="gbtn">${ic('up')}Replier</span><span class="gbtn">${ic('x')}Quitter</span></div>
    ${v3banner(on)}${on ? `<div class="hint">${CLICK_HINT}</div>` : ''}
    ${s.multi ? v3list() : ''}
    <div class="set">${s.multi ? `<div class="sect">Réglages de «${NBSP}${OURS.name}${NBSP}»</div>` : ''}
      <div class="lab">Bouton maintenu</div>${seg(BUTTONS, 'Gauche')}${key}${s.refused ? warnBox() : ''}
    </div>
  </div>`;
}

function v3banner(on) {
  return on
    ? `<div class="ban on"><div class="bt"><span class="bico">${ic('dot')}</span><div><div class="big">ACTIVE</div><div class="sub">${OURS.name}</div><div class="sub b">Bouton gauche enfoncé</div></div></div>
       <div class="act">${ic('stop')}Arrêter${kc(OURS.key)}</div></div>`
    : `<div class="ban"><div class="bt"><span class="bico">${ic('stop')}</span><div><div class="big">ARRÊTÉE</div><div class="sub">${OURS.name}</div><div class="sub">Bouton gauche</div></div></div>
       <div class="act">${ic('play')}Activer${kc(OURS.key)}</div></div>`;
}

function v3list() {
  const soon = (m) =>
    `<div class="li soon">${ic(m.icon)}<span class="name">${m.name}</span>${kc(m.key, 'off')}<span class="stt">À venir</span></div>`;
  return `<div class="list">
    <div class="li sel">${ic('mouse')}<span class="name">${OURS.name}</span>${kc(OURS.key)}<span class="stt">${ic('stop')}Arrêtée</span></div>
    ${SOON.map(soon).join('')}
  </div>`;
}

function v3mini(on) {
  return `<div class="hud v3 light mini${on ? ' on' : ''}">${ic('grip', 'grip')}<span class="bico">${ic(on ? 'dot' : 'stop')}</span>
    <div><div class="big">${on ? 'ACTIVE' : 'ARRÊTÉE'}</div><div class="sub">${on ? 'Bouton gauche enfoncé' : OURS.name}</div></div>
    <span class="sp"></span>${kc(OURS.key)}<span class="gbtn">${ic('down')}Déplier</span></div>`;
}

// ---------------------------------------------------------------------------
// Variant 4 — Souris
// ---------------------------------------------------------------------------

function v4(s) {
  if (s.collapsed) return v4mini(false) + v4mini(true);
  const on = s.state === 'active';
  const state = on
    ? `<div class="state on">${ic('dot')}Active</div><div class="sub">Bouton gauche enfoncé</div>`
    : `<div class="state">${ic('stop')}Arrêtée</div><div class="sub">${OURS.desc}</div>`;
  const key = s.capture
    ? `<div class="keyrow">${kc('…', 'big wait')}<span class="sbtn">Annuler</span></div><div class="capt"><div class="t1">${CAPTURE.t1}</div><div class="t2">${CAPTURE.t2}</div></div>`
    : `<div class="keyrow">${kc(OURS.key, 'big')}<span class="sbtn">Changer</span></div>`;
  const action = on ? `<div class="pbtn">${ic('stop')}Arrêter</div>` : `<div class="pbtn">${ic('play')}Activer</div>`;
  return `<div class="hud v4 dark${on ? ' on' : ''}">
    <div class="bar">${ic('grip', 'grip')}<span class="brand">MacroHUD</span><span class="ver">${VERSION}</span><span class="sp"></span><span class="gbtn">${ic('up')}Replier</span><span class="gbtn">${ic('x')}Quitter</span></div>
    ${s.multi ? v4tabs() : ''}
    <div class="body">
      <div class="figbox">${mouseFig({ sel: 'Gauche', on })}</div>
      <div class="right"><div class="name">${OURS.name}</div>${state}
        <div class="lab">Raccourci</div>${key}${action}${on ? `<div class="hint">${CLICK_HINT}</div>` : ''}
      </div>
    </div>
    ${s.refused ? `<div class="mwrap">${warnBox()}</div>` : ''}
    <div class="bottom"><div class="lab">Bouton maintenu</div>${seg(['Gauche', 'Milieu', 'Droit'], 'Gauche')}</div>
  </div>`;
}

function v4tabs() {
  const soon = (m) =>
    `<div class="tab soon"><div class="tr">${ic(m.icon, 'ti')}<span class="sp"></span>${kc(m.key, 'sm off')}</div><div class="tn">${m.name}</div><div class="ts">À venir</div></div>`;
  return `<div class="tabs">
    <div class="tab sel"><div class="tr">${mouseMini(false)}<span class="sp"></span>${kc(OURS.key, 'sm')}</div><div class="tn">${OURS.name}</div><div class="ts">${ic('stop')}Arrêtée</div></div>
    ${SOON.map(soon).join('')}
  </div>`;
}

function v4mini(on) {
  return `<div class="hud v4 dark mini${on ? ' on' : ''}">${ic('grip', 'grip')}${mouseMini(on)}
    <div class="mt"><div class="ms">${ic(on ? 'dot' : 'stop')}${on ? 'Active' : 'Arrêtée'}</div><div class="mb">Bouton gauche</div></div>
    ${kc(OURS.key)}<span class="gbtn">${ic('down')}Déplier</span></div>`;
}

// ---------------------------------------------------------------------------
// Variant 5 — Colonnes
// ---------------------------------------------------------------------------

function v5(s) {
  if (s.collapsed) return v5mini('stopped') + v5mini('active') + v5mini('multi');
  let cols = v5col(s);
  if (s.multi) cols += SOON.map(v5soon).join('');
  return `<div class="hud v5 dark${s.multi ? '' : ' single'}">
    <div class="bar">${ic('grip', 'grip')}<span class="brand">MacroHUD</span><span class="ver">${VERSION}</span></div>
    <div class="cols">${cols}</div>
    <div class="foot"><span class="tbtn">${ic('up')}Replier</span><span class="tbtn">${ic('x')}Quitter</span></div>
  </div>`;
}

function v5col(s) {
  const on = s.state === 'active';
  const top = s.capture
    ? `${kc('…', 'big wait')}<div class="ct1">${CAPTURE.t1}</div><div class="ct2">${CAPTURE.t2}</div><span class="chg">Annuler</span>`
    : `${kc(OURS.key, 'big')}<span class="chg">Changer</span>`;
  const pill = on
    ? `<div class="pill on">${ic('dot')}Active</div><div class="sub">Gauche enfoncé</div>`
    : `<div class="pill">${ic('stop')}Arrêtée</div>`;
  const radios = BUTTONS.map((b) => `<div class="ra${b === 'Gauche' ? ' sel' : ''}"><span class="dot"></span>${b}</div>`).join('');
  const action = on ? `<div class="pbtn">${ic('stop')}Arrêter</div>` : `<div class="pbtn">${ic('play')}Activer</div>`;
  return `<div class="col${on ? ' on' : ''}">${top}${s.refused ? warnBox(false) : ''}
    <div class="name">${OURS.name}</div>${pill}<div class="lab">Bouton</div><div class="radios">${radios}</div>${action}</div>`;
}

function v5soon(m) {
  return `<div class="col soon">${kc(m.key, 'big off')}<span class="chg off">Changer</span>
    <div class="name">${m.name}</div><div class="pill soon">À venir</div>
    <div class="lab">${m.optLabel}</div><div class="val">${m.optValue}</div><div class="pbtn off">${ic('play')}Activer</div></div>`;
}

function v5mini(kind) {
  if (kind === 'multi') {
    return `<div class="hud v5 dark mini">${ic('grip', 'grip')}<span class="kc on">${ic('dot')}F8</span>${kc('F9', 'off')}${kc('F10', 'off')}<span class="ibtn">${ic('down')}</span></div>`;
  }
  const on = kind === 'active';
  return `<div class="hud v5 dark mini">${ic('grip', 'grip')}${kc(OURS.key)}<span class="pill${on ? ' on' : ''}">${ic(on ? 'dot' : 'stop')}${on ? 'Active' : 'Arrêtée'}</span><span class="ibtn">${ic('down')}</span></div>`;
}

// ---------------------------------------------------------------------------
// Variants
// ---------------------------------------------------------------------------

const VARIANTS = [
  {
    n: 1,
    title: 'Cartes',
    idea: 'Un panneau vertical où chaque macro est une carte complète : tout est visible sans clic (état en pastille, bouton maintenu, raccourci, bouton Activer). Active : la carte se borde de vert.',
    build: v1,
  },
  {
    n: 2,
    title: 'Liste compacte',
    idea: 'Une ligne par macro : à gauche le bouton ▶ et l’état, à droite la touche du raccourci. Les réglages s’ouvrent sous la ligne avec la flèche. Active : toute la ligne devient verte.',
    build: v2,
  },
  {
    n: 3,
    title: 'Grand état',
    idea: 'Thème clair. Un grand bandeau d’état en haut, lisible de loin, avec le bouton Activer dedans ; les réglages dessous. Repliée, il ne reste que le bandeau.',
    build: v3,
  },
  {
    n: 4,
    title: 'Souris',
    idea: 'Une souris dessinée : on choisit le bouton sur le dessin (ou dessous), et le bouton enfoncé s’allume en vert avec une flèche. Le raccourci en grande touche à côté. Active : contour vert.',
    build: v4,
  },
  {
    n: 5,
    title: 'Colonnes',
    idea: 'Une colonne par macro, côte à côte comme une télécommande, chacune coiffée de sa touche de raccourci. Repliée : une rangée de touches, une par macro.',
    build: v5,
    fLabel: 'Repliée : arrêtée, active, 3 macros',
  },
];

// ---------------------------------------------------------------------------
// Icons and background scenes (inline SVG sprite)
// ---------------------------------------------------------------------------

const SPRITE = `<svg width="0" height="0" style="position:absolute" aria-hidden="true">
<defs>
  <linearGradient id="g-nuit" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#060a16"/><stop offset="1" stop-color="#1e2b48"/></linearGradient>
  <radialGradient id="g-lune"><stop offset="0" stop-color="#fff8dc" stop-opacity=".55"/><stop offset="1" stop-color="#fff8dc" stop-opacity="0"/></radialGradient>
  <linearGradient id="g-ciel" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4fa8f5"/><stop offset="1" stop-color="#d2ecff"/></linearGradient>
  <pattern id="p-herbe" width="9" height="9" patternUnits="userSpaceOnUse"><rect width="9" height="9" fill="#4d9a38"/><path d="M1.5 9 3 3M5.5 9 7 1.5" stroke="#86d062" stroke-width="1.6"/></pattern>
  <pattern id="p-texte" width="300" height="18" patternUnits="userSpaceOnUse"><rect x="0" y="6" width="210" height="6" rx="2" fill="#30343b"/><rect x="220" y="6" width="60" height="6" rx="2" fill="#30343b"/></pattern>
</defs>
<symbol id="i-grip" viewBox="0 0 16 16"><g fill="currentColor"><circle cx="5.5" cy="3.5" r="1.5"/><circle cx="10.5" cy="3.5" r="1.5"/><circle cx="5.5" cy="8" r="1.5"/><circle cx="10.5" cy="8" r="1.5"/><circle cx="5.5" cy="12.5" r="1.5"/><circle cx="10.5" cy="12.5" r="1.5"/></g></symbol>
<symbol id="i-up" viewBox="0 0 16 16"><path d="M3 10.5 8 5.5l5 5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></symbol>
<symbol id="i-down" viewBox="0 0 16 16"><path d="M3 5.5 8 10.5l5-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></symbol>
<symbol id="i-x" viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/></symbol>
<symbol id="i-play" viewBox="0 0 16 16"><path d="M4.5 2.5v11L13.5 8z" fill="currentColor" stroke="currentColor" stroke-width="1" stroke-linejoin="round"/></symbol>
<symbol id="i-stop" viewBox="0 0 16 16"><rect x="3" y="3" width="10" height="10" rx="1.5" fill="currentColor"/></symbol>
<symbol id="i-dot" viewBox="0 0 16 16"><circle cx="8" cy="8" r="5.5" fill="currentColor"/></symbol>
<symbol id="i-check" viewBox="0 0 16 16"><path d="M2.5 8.5l3.5 3.5 7.5-8" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></symbol>
<symbol id="i-warn" viewBox="0 0 16 16"><path d="M8 1.2 15.4 14.6H.6z" fill="currentColor" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/><rect x="7.1" y="5.4" width="1.8" height="5" rx=".9" fill="#1a1206"/><circle cx="8" cy="12.3" r="1.05" fill="#1a1206"/></symbol>
<symbol id="i-mouse" viewBox="0 0 16 16"><rect x="3.5" y="1" width="9" height="14" rx="4.5" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M8 1V6.5H3.5V5.5A4.5 4.5 0 0 1 8 1Z" fill="currentColor"/><path d="M3.5 6.5h9" stroke="currentColor" stroke-width="1.6"/></symbol>
<symbol id="i-repeat" viewBox="0 0 16 16"><rect x="5.5" y="2" width="9" height="13" rx="4.5" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M10 2v5M5.5 7h9" stroke="currentColor" stroke-width="1.6"/><path d="M1 3.2l2.4 1.2M.6 7h2.6M1 10.8l2.4-1.2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></symbol>
<symbol id="i-key" viewBox="0 0 16 16"><rect x="1.2" y="2.2" width="13.6" height="11.6" rx="2.4" fill="none" stroke="currentColor" stroke-width="1.6"/><rect x="4" y="4.6" width="8" height="5.2" rx="1.2" fill="currentColor"/></symbol>

<symbol id="sc-bureau" viewBox="0 0 480 440" preserveAspectRatio="xMidYMid slice">
  <rect width="480" height="440" fill="#eceef1"/>
  <rect width="480" height="42" fill="#f9fafb"/><rect y="42" width="480" height="1" fill="#cfd4da"/>
  <g fill="#c7ccd4"><rect x="12" y="11" width="20" height="20" rx="4"/><rect x="38" y="11" width="20" height="20" rx="4"/><rect x="64" y="11" width="20" height="20" rx="4"/><rect x="96" y="11" width="44" height="20" rx="4"/></g>
  <g><rect x="290" y="13" width="16" height="16" rx="3" fill="#111"/><rect x="310" y="13" width="16" height="16" rx="3" fill="#e5484d"/><rect x="330" y="13" width="16" height="16" rx="3" fill="#f59e0b"/><rect x="350" y="13" width="16" height="16" rx="3" fill="#facc15"/><rect x="370" y="13" width="16" height="16" rx="3" fill="#22c55e"/><rect x="390" y="13" width="16" height="16" rx="3" fill="#3b82f6"/><rect x="410" y="13" width="16" height="16" rx="3" fill="#8b5cf6"/><rect x="430" y="13" width="16" height="16" rx="3" fill="#fff" stroke="#b9bfc8"/></g>
  <rect x="14" y="56" width="452" height="370" fill="#ffffff" stroke="#cfd4da"/>
  <rect x="34" y="76" width="150" height="74" fill="#fde68a"/>
  <rect x="34" y="168" width="300" height="108" fill="url(#p-texte)"/>
  <circle cx="392" cy="122" r="44" fill="none" stroke="#dc2626" stroke-width="6"/>
  <path d="M36 360C90 270 150 410 212 318S330 240 392 350" fill="none" stroke="#2563eb" stroke-width="8" stroke-linecap="round"/>
  <path d="M250 400l60-46 34 30 50-58" fill="none" stroke="#16a34a" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
</symbol>

<symbol id="sc-nuit" viewBox="0 0 480 440" preserveAspectRatio="xMidYMid slice">
  <rect width="480" height="440" fill="url(#g-nuit)"/>
  <circle cx="396" cy="72" r="70" fill="url(#g-lune)"/><circle cx="396" cy="72" r="22" fill="#efe9cf"/>
  <g fill="#c8d2ff"><circle cx="40" cy="40" r="1.6"/><circle cx="92" cy="86" r="1.2"/><circle cx="150" cy="30" r="1.8"/><circle cx="210" cy="70" r="1.2"/><circle cx="260" cy="24" r="1.5"/><circle cx="318" cy="110" r="1.2"/><circle cx="30" cy="140" r="1.3"/><circle cx="120" cy="160" r="1.6"/><circle cx="452" cy="160" r="1.4"/><circle cx="350" cy="40" r="1.2"/></g>
  <path d="M0 290 80 214l70 52 80-86 92 104 76-58 82 52V440H0Z" fill="#162038"/>
  <path d="M0 330 60 300l70 26 90-44 80 36 60-22 120 40V440H0Z" fill="#0f1729"/>
  <path d="M300 330V262h14v-16h12v16h20v-34h16v34h20v-16h12v16h14v68Z" fill="#0a0f1c"/>
  <g fill="#ffc45e"><rect x="318" y="276" width="6" height="9"/><rect x="352" y="250" width="6" height="9"/><rect x="384" y="282" width="6" height="9"/><rect x="336" y="300" width="6" height="9"/></g>
  <path d="M0 372Q120 346 240 372T480 360V440H0Z" fill="#08100c"/>
  <rect x="16" y="404" width="156" height="16" rx="4" fill="#141414" stroke="#000"/><rect x="19" y="407" width="108" height="10" rx="2" fill="#d23c3c"/>
  <rect x="16" y="382" width="110" height="14" rx="4" fill="#141414" stroke="#000"/><rect x="19" y="385" width="70" height="8" rx="2" fill="#3d7ff2"/>
</symbol>

<symbol id="sc-jour" viewBox="0 0 480 440" preserveAspectRatio="xMidYMid slice">
  <rect width="480" height="440" fill="url(#g-ciel)"/>
  <circle cx="74" cy="66" r="30" fill="#fff3a8"/>
  <g fill="#ffffff"><ellipse cx="220" cy="70" rx="56" ry="18"/><ellipse cx="258" cy="56" rx="36" ry="20"/><ellipse cx="400" cy="120" rx="50" ry="15"/></g>
  <path d="M0 250Q120 196 240 240T480 226V440H0Z" fill="#7cc95c"/>
  <path d="M0 300Q140 262 300 300T480 290V440H0Z" fill="#3f8d36"/>
  <rect x="300" y="236" width="70" height="52" fill="#f1e3c6"/><path d="M292 240l43-34 43 34Z" fill="#d34f45"/><rect x="326" y="258" width="18" height="30" fill="#7a4b2a"/>
  <g fill="#6a4426"><rect x="96" y="250" width="8" height="34"/><rect x="186" y="262" width="8" height="30"/><rect x="430" y="246" width="8" height="34"/></g>
  <g fill="#2d6a28"><circle cx="100" cy="236" r="26"/><circle cx="190" cy="248" r="22"/><circle cx="434" cy="232" r="26"/></g>
  <path d="M150 440C190 380 250 350 330 322" fill="none" stroke="#d8b679" stroke-width="34"/>
  <rect y="372" width="480" height="68" fill="url(#p-herbe)"/>
  <g><circle cx="430" cy="404" r="14" fill="#f5c542" stroke="#8a6a12" stroke-width="3"/><rect x="340" y="396" width="70" height="16" rx="4" fill="#20232a"/><rect x="343" y="399" width="50" height="10" rx="2" fill="#f5c542"/></g>
</symbol>
</svg>`;

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const CSS = `
@font-face{font-family:"Inter";src:url(data:font/woff2;base64,${FONT}) format("woff2");font-weight:100 900;font-style:normal;font-display:block}
*{box-sizing:border-box;margin:0;padding:0}
html{background:#e7e5e0}
body{font-family:"Inter",system-ui,sans-serif;color:#1c1d20;-webkit-font-smoothing:antialiased;max-width:1600px;margin:0 auto;padding:40px 40px 56px}

/* ---------- page ---------- */
.page-head h1{font-size:34px;font-weight:800;letter-spacing:-.01em}
.page-head p{font-size:17px;line-height:1.5;color:#3f4147;margin-top:8px;max-width:1180px}
.legend{display:flex;flex-wrap:wrap;gap:8px 18px;margin-top:14px;font-size:15px;color:#2b2d32;list-style:none}
.legend b{display:inline-flex;width:22px;height:22px;border-radius:6px;background:#1c1d20;color:#fff;align-items:center;justify-content:center;font-size:13px;margin-right:6px}
.page-head .note{font-size:14px;color:#55585f}
.variant{background:#f7f6f3;border:1px solid #d3d0c9;border-radius:18px;padding:26px 28px 30px;margin-top:28px}
.vhead{display:flex;gap:16px;align-items:center;margin-bottom:20px}
.vnum{flex:none;width:56px;height:56px;border-radius:14px;background:#1c1d20;color:#fff;font-size:32px;font-weight:800;display:flex;align-items:center;justify-content:center}
.vhead h2{font-size:28px;font-weight:800;letter-spacing:-.01em}
.vhead p{font-size:16px;line-height:1.45;color:#43464c;margin-top:3px;max-width:1260px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(440px,1fr));gap:20px}
.tile figcaption{display:flex;align-items:center;gap:8px;margin-bottom:8px;font-size:15px;font-weight:600}
.tile figcaption b{flex:none;display:inline-flex;width:24px;height:24px;border-radius:6px;background:#1c1d20;color:#fff;align-items:center;justify-content:center;font-size:14px}
.tile .bgname{margin-left:auto;font-weight:500;font-size:13px;color:#686b72}
.stage{position:relative;border-radius:12px;overflow:hidden;min-height:380px;height:calc(100% - 32px);display:flex;align-items:center;justify-content:center;padding:28px 20px}
.stage>.scene{position:absolute;inset:0;width:100%;height:100%}
.stage>.place{position:relative;display:flex;flex-direction:column;align-items:center;gap:18px}
.foot-note{margin-top:28px;font-size:14px;color:#55585f;line-height:1.5}
@media (max-width:720px){
  body{padding:16px 12px 32px}
  .page-head h1{font-size:26px}
  .variant{padding:16px 12px 20px;border-radius:14px}
  .vhead{align-items:flex-start}
  .vnum{width:44px;height:44px;font-size:26px}
  .vhead h2{font-size:22px}
  .grid{grid-template-columns:1fr}
  .tile figcaption{flex-wrap:wrap}
  .tile .bgname{margin-left:32px;width:100%}
  .stage{padding:20px 8px}
  .stage>.place{zoom:.82}
}

/* ---------- HUD base: no kerning or ligatures, like egui ---------- */
.hud{font-family:"Inter";font-kerning:none;font-variant-ligatures:none;font-feature-settings:"calt" 0;line-height:1.25;text-align:left;white-space:normal}
.hud .ic{display:block;flex:none;width:16px;height:16px}
.hud .sp{flex:1}
.kc{display:inline-flex;align-items:center;justify-content:center;flex:none;min-width:34px;height:28px;padding:0 8px 3px;border-radius:6px;background:#e9edf2;color:#111418;font-size:14px;font-weight:800;line-height:1;box-shadow:inset 0 -3px 0 #9aa4b1}
.dark .kc.off{background:#2d323a;color:#a3abb6;box-shadow:inset 0 -3px 0 #1f2329}
.kc.wait{min-width:40px;background:transparent;color:#6cb8ff;box-shadow:none;outline:2px dashed #6cb8ff;outline-offset:-2px;padding-bottom:0}

/* shared dark blocks: capture and refused message */
.dark .cap{margin-top:12px;padding:10px;border-radius:8px;background:#10233a;border:1px solid #6cb8ff}
.dark .cap .c1{display:flex;align-items:center;gap:10px}
.dark .cap .c2{display:flex;align-items:center;gap:8px;margin-top:6px;padding-left:50px}
.dark .cap .t1{font-size:14px;font-weight:600;color:#f3f5f8}
.dark .cap .t2{flex:1;font-size:12px;color:#b3d7ff}
.dark .msg{margin-top:10px;display:flex;gap:10px;padding:10px 12px 12px 10px;border-radius:8px;background:#2b2112;border:1px solid #ffb547}
.dark .msg .wi{width:18px;height:18px;color:#ffb547;margin-top:1px}
.dark .msg .t1{font-size:14px;font-weight:700;color:#fff1d6}
.dark .msg .t2{font-size:12px;line-height:1.35;color:#ffe2b0;margin-top:3px}
.dark .msg .abtn{margin-top:8px;height:28px;display:inline-flex;align-items:center;padding:0 10px;border-radius:6px;border:1px solid #ffb547;background:#3a2c14;color:#fff1d6;font-size:12px;font-weight:600}

/* ---------- V1 Cartes ---------- */
.v1.hud{width:320px;background:#14171c;color:#f3f5f8;border:1px solid #5d6876;border-radius:12px;box-shadow:0 0 0 1px #000,0 8px 22px rgba(0,0,0,.45)}
.v1 .bar{height:40px;display:flex;align-items:center;gap:8px;padding:0 6px 0 8px;border-bottom:1px solid #2a3039}
.v1 .grip{width:14px;height:18px;color:#8a94a2}
.v1 .brand{font-size:14px;font-weight:700}
.v1 .ver{font-size:12px;font-weight:500;color:#b3bcc8}
.v1 .gbtn{height:28px;display:inline-flex;align-items:center;gap:6px;padding:0 10px;border-radius:6px;background:#252b34;border:1px solid #4a5361;color:#e6eaf0;font-size:12px;font-weight:600}
.v1 .gbtn .ic,.v1 .tbtn .ic{width:12px;height:12px}
.v1 .body{padding:10px}
.v1 .card{background:#1d2128;border:1px solid #343b46;border-radius:10px;padding:12px}
.v1 .card.on{border:2px solid #34d27c;padding:11px}
.v1 .head{display:flex;align-items:center;gap:8px}
.v1 .name{flex:1;font-size:16px;font-weight:700}
.v1 .badge{flex:none;height:24px;display:inline-flex;align-items:center;gap:6px;padding:0 9px 0 8px;border-radius:12px;background:#2b313b;border:1px solid #6b7685;color:#e6eaf0;font-size:12px;font-weight:700;letter-spacing:.04em}
.v1 .badge .ic{width:10px;height:10px}
.v1 .badge.on{background:#34d27c;border-color:#34d27c;color:#04140a}
.v1 .desc{font-size:12px;color:#b3bcc8;margin-top:3px}
.v1 .desc.on{color:#5fe3a0;font-weight:600}
.v1 .lab{font-size:12px;font-weight:600;color:#b3bcc8}
.v1 .lab.mt{margin-top:14px}
.v1 .seg{margin-top:6px;display:grid;grid-template-columns:repeat(3,1fr);gap:2px;height:34px;padding:2px;background:#14171c;border:1px solid #5d6876;border-radius:8px}
.v1 .seg span{display:flex;align-items:center;justify-content:center;gap:5px;border-radius:6px;font-size:14px;font-weight:500;color:#e6eaf0}
.v1 .seg span.sel{background:#e9edf2;color:#111418;font-weight:700}
.v1 .seg .ic{width:12px;height:12px}
.v1 .row{display:flex;align-items:center;gap:8px;margin-top:12px}
.v1 .sbtn{height:28px;display:inline-flex;align-items:center;padding:0 10px;border-radius:6px;background:#2b313b;border:1px solid #6b7685;color:#f3f5f8;font-size:14px;font-weight:600}
.v1 .cap .sbtn{height:26px;font-size:12px}
.v1 .pbtn{margin-top:14px;height:38px;display:flex;align-items:center;gap:8px;padding:0 6px 0 12px;border-radius:8px;background:#f3f5f8;color:#111418;font-size:14px;font-weight:700}
.v1 .pbtn .ic{width:12px;height:12px}
.v1 .pbtn .kc{margin-left:auto;height:26px;min-width:32px;background:#fff;box-shadow:inset 0 0 0 1px #9aa4b1,inset 0 -3px 0 #9aa4b1}
.v1 .hint{margin-top:8px;text-align:center;font-size:12px;color:#b3bcc8}
.v1 .foot{display:flex;justify-content:flex-end;padding:0 10px 10px}
.v1 .tbtn{height:28px;display:inline-flex;align-items:center;gap:6px;padding:0 8px;border-radius:6px;color:#c9d1dc;font-size:12px;font-weight:600}
.v1 .card.soon{margin-top:8px;background:#181b20;border:1px solid #2c323b;padding:10px 12px}
.v1 .card.soon .name{font-size:14px;font-weight:600;color:#7a838f}
.v1 .card.soon .ti{color:#7a838f}
.v1 .badge.soon{background:transparent;border:1px solid #6b7480;color:#a3abb6}
.v1 .srow{display:flex;align-items:center;gap:8px;margin-top:8px;font-size:12px;color:#7a838f}
.v1 .srow .kc{height:24px;font-size:12px;min-width:30px;padding:0 6px 3px}
.v1.mini{width:auto;height:44px;display:flex;align-items:center;gap:8px;padding:0 7px 0 8px;border-radius:22px}
.v1.mini.on{border:2px solid #34d27c;padding:0 6px 0 7px}
.v1.mini .name{flex:none;font-size:14px;font-weight:600}
.v1 .ibtn{width:30px;height:30px;border-radius:15px;display:inline-flex;align-items:center;justify-content:center;background:#252b34;border:1px solid #4a5361;color:#e6eaf0}
.v1 .ibtn .ic{width:14px;height:14px}

/* ---------- V2 Liste compacte ---------- */
.v2.hud{width:288px;background:#0f1114;color:#f3f5f8;border:1px solid #5d6876;border-radius:8px;box-shadow:0 0 0 1px #000,0 6px 16px rgba(0,0,0,.5);overflow:hidden}
.v2 .bar{height:32px;display:flex;align-items:center;gap:6px;padding:0 4px 0 6px;border-bottom:1px solid #2a2f37}
.v2 .grip{width:12px;height:16px;color:#7f8996}
.v2 .brand{font-size:12px;font-weight:700}
.v2 .ver{font-size:12px;color:#aab3bf}
.v2 .tbtn{height:24px;display:inline-flex;align-items:center;gap:4px;padding:0 6px;border-radius:5px;color:#d3d9e1;font-size:12px;font-weight:600}
.v2 .tbtn .ic{width:10px;height:10px}
.v2 .row{height:50px;display:flex;align-items:center;gap:8px;padding-right:6px;background:#181b20;border-bottom:1px solid #262b33}
.v2 .stripe{align-self:stretch;width:4px;flex:none;background:#6b7685}
.v2 .tog{flex:none;width:32px;height:32px;margin-left:2px;border-radius:8px;display:flex;align-items:center;justify-content:center;background:#2b313b;border:1px solid #6b7685;color:#f3f5f8}
.v2 .tog .ic{width:13px;height:13px}
.v2 .tico{flex:none;width:32px;height:32px;margin-left:2px;display:flex;align-items:center;justify-content:center;color:#7a838f}
.v2 .txt{flex:1;min-width:0}
.v2 .name{font-size:14px;font-weight:600;white-space:nowrap}
.v2 .st{display:flex;align-items:center;gap:4px;margin-top:2px;font-size:12px;color:#aab3bf;white-space:nowrap}
.v2 .st .ic{width:9px;height:9px}
.v2 .kc{height:24px;min-width:30px;padding:0 6px 3px;font-size:12px}
.v2 .dis{flex:none;width:24px;height:24px;border-radius:5px;display:flex;align-items:center;justify-content:center;color:#d3d9e1}
.v2 .dis .ic{width:13px;height:13px}
.v2 .dis.off{color:#4f5762}
.v2 .row.on{background:#34d27c;border-bottom-color:#1f9b57}
.v2 .row.on .stripe{background:#04140a}
.v2 .row.on .tog{background:#04140a;border-color:#04140a;color:#34d27c}
.v2 .row.on .name,.v2 .row.on .st,.v2 .row.on .dis{color:#04140a}
.v2 .row.on .st{font-weight:600}
.v2 .row.on .kc{background:#fff;box-shadow:inset 0 -3px 0 #1f9b57}
.v2 .row.soon{background:#14161a}
.v2 .row.soon .stripe{background:#30353d}
.v2 .row.soon .name,.v2 .row.soon .st{color:#7a838f}
.v2 .opts{padding:8px 8px 10px 12px;background:#14171b;border-bottom:1px solid #262b33}
.v2 .lab{font-size:12px;font-weight:600;color:#aab3bf}
.v2 .seg{margin-top:4px;display:grid;grid-template-columns:repeat(3,1fr);gap:2px;height:30px;padding:2px;background:#0f1114;border:1px solid #5d6876;border-radius:7px}
.v2 .seg span{display:flex;align-items:center;justify-content:center;gap:4px;border-radius:5px;font-size:14px;font-weight:500;color:#e6eaf0}
.v2 .seg span.sel{background:#e9edf2;color:#111418;font-weight:700}
.v2 .seg .ic{width:11px;height:11px}
.v2 .orow{display:flex;align-items:center;gap:6px;margin-top:10px}
.v2 .sbtn{height:26px;display:inline-flex;align-items:center;padding:0 9px;border-radius:6px;background:#2b313b;border:1px solid #6b7685;color:#f3f5f8;font-size:12px;font-weight:600}
.v2 .cap{margin-top:10px}
.v2 .cap .c2{padding-left:0}
.v2 .msg{margin-top:8px}
.v2.mini{width:auto;height:34px;display:flex;align-items:center;gap:6px;padding:0 4px 0 6px;border-radius:17px}
.v2.mini.on{background:#34d27c;color:#04140a}
.v2.mini.on .grip,.v2.mini.on .dis{color:#04140a}
.v2.mini.on .kc{background:#fff;box-shadow:inset 0 -3px 0 #1f9b57}
.v2 .mst{display:inline-flex;align-items:center;gap:5px;font-size:12px;font-weight:700;margin-right:2px}
.v2 .mst .ic{width:10px;height:10px}

/* ---------- V3 Grand état (light) ---------- */
.v3.hud{width:320px;background:#fff;color:#111418;border:1px solid #1b1f24;border-radius:14px;box-shadow:0 10px 26px rgba(0,0,0,.38)}
.v3 .bar{height:44px;display:flex;align-items:center;gap:6px;padding:0 8px 0 8px}
.v3 .grip{width:14px;height:18px;color:#6b7480}
.v3 .brand{font-size:14px;font-weight:800;margin-left:2px}
.v3 .ver{font-size:12px;font-weight:500;color:#4a5462}
.v3 .gbtn{height:28px;display:inline-flex;align-items:center;gap:5px;padding:0 8px;border-radius:7px;background:#fff;border:1px solid #6b7480;color:#111418;font-size:12px;font-weight:600}
.v3 .gbtn .ic{width:11px;height:11px}
.v3 .ban{margin:0 10px;padding:12px;border-radius:12px;background:#e4e8ee}
.v3 .bt{display:flex;align-items:center;gap:12px}
.v3 .bico{flex:none;width:42px;height:42px;border-radius:21px;display:flex;align-items:center;justify-content:center;background:#fff;border:2px solid #4a5462;color:#4a5462}
.v3 .bico .ic{width:15px;height:15px}
.v3 .big{font-size:20px;font-weight:800;letter-spacing:.02em;line-height:1.15}
.v3 .sub{font-size:12px;color:#3d4653;margin-top:1px}
.v3 .sub.b{font-weight:700}
.v3 .act{margin-top:12px;height:38px;display:flex;align-items:center;gap:8px;padding:0 6px 0 12px;border-radius:9px;background:#111418;color:#fff;font-size:14px;font-weight:700}
.v3 .act .ic{width:12px;height:12px}
.v3 .act .kc{margin-left:auto;height:26px;min-width:32px;background:#fff;color:#111418;box-shadow:inset 0 -3px 0 #8b94a1}
.v3 .ban.on{background:#137a3c;color:#fff}
.v3 .ban.on .bico{background:#137a3c;border-color:#fff;color:#fff}
.v3 .ban.on .sub{color:#ffffff}
.v3 .ban.on .act{background:#fff;color:#0d5a2b}
.v3 .ban.on .act .kc{box-shadow:inset 0 0 0 1px #0d5a2b,inset 0 -3px 0 #0d5a2b}
.v3 .hint{margin:8px 12px 0;text-align:center;font-size:12px;color:#3d4653}
.v3 .set{padding:12px}
.v3 .sect{font-size:12px;font-weight:700;color:#111418;margin-bottom:8px}
.v3 .lab{font-size:12px;font-weight:700;color:#4a5462}
.v3 .seg{margin-top:6px;display:grid;grid-template-columns:repeat(3,1fr);gap:2px;height:36px;padding:2px;background:#f1f3f6;border:1px solid #6b7480;border-radius:9px}
.v3 .seg span{display:flex;align-items:center;justify-content:center;gap:5px;border-radius:7px;font-size:14px;font-weight:500;color:#111418}
.v3 .seg span.sel{background:#111418;color:#fff;font-weight:700}
.v3 .seg .ic{width:12px;height:12px}
.v3 .row{display:flex;align-items:center;gap:8px;margin-top:12px}
.v3 .kc{background:#fff;color:#111418;box-shadow:inset 0 0 0 1px #6b7480,inset 0 -3px 0 #6b7480}
.v3 .kc.off{background:#f1f3f6;color:#5f6874;box-shadow:inset 0 0 0 1px #b8c0ca,inset 0 -3px 0 #b8c0ca}
.v3 .kc.wait{color:#1d5fa8;outline-color:#1d5fa8;box-shadow:none;background:transparent}
.v3 .sbtn{height:30px;display:inline-flex;align-items:center;padding:0 12px;border-radius:7px;background:#fff;border:1px solid #6b7480;color:#111418;font-size:14px;font-weight:600}
.v3 .cap{margin-top:12px;padding:10px;border-radius:9px;background:#e8f1fc;border:1px solid #1d5fa8}
.v3 .cap .c1{display:flex;align-items:center;gap:10px}
.v3 .cap .c2{display:flex;align-items:center;gap:8px;margin-top:6px;padding-left:50px}
.v3 .cap .t1{font-size:14px;font-weight:700;color:#0b2e57}
.v3 .cap .t2{flex:1;font-size:12px;color:#1d4a7f}
.v3 .cap .sbtn{height:26px;font-size:12px;padding:0 9px}
.v3 .msg{margin-top:10px;display:flex;gap:10px;padding:10px 12px 12px 10px;border-radius:9px;background:#fff4dc;border:1px solid #b7791f}
.v3 .msg .wi{width:18px;height:18px;color:#c47f0e;margin-top:1px}
.v3 .msg .t1{font-size:14px;font-weight:700;color:#3d2a00}
.v3 .msg .t2{font-size:12px;line-height:1.35;color:#5c3d00;margin-top:3px}
.v3 .msg .abtn{margin-top:8px;height:28px;display:inline-flex;align-items:center;padding:0 10px;border-radius:7px;border:1px solid #b7791f;background:#fff;color:#3d2a00;font-size:12px;font-weight:600}
.v3 .list{margin:12px 10px 0;border:1px solid #b8c0ca;border-radius:10px;overflow:hidden}
.v3 .li{height:40px;display:flex;align-items:center;gap:8px;padding:0 10px;border-top:1px solid #dde2e8}
.v3 .li:first-child{border-top:0}
.v3 .li.sel{background:#eef1f5;box-shadow:inset 4px 0 0 #111418}
.v3 .li .name{flex:1;font-size:14px;font-weight:600}
.v3 .li .kc{height:24px;min-width:30px;padding:0 6px 3px;font-size:12px}
.v3 .li .stt{width:66px;display:flex;align-items:center;gap:5px;font-size:12px;font-weight:600;color:#3d4653}
.v3 .li .stt .ic{width:9px;height:9px}
.v3 .li.soon .name,.v3 .li.soon .stt,.v3 .li.soon>.ic{color:#5f6874}
.v3.mini{width:auto;display:flex;align-items:center;gap:10px;padding:8px 8px 8px 8px;border-radius:14px}
.v3.mini .bico{width:36px;height:36px}
.v3.mini .big{font-size:20px}
.v3.mini.on{background:#137a3c;color:#fff;border-color:#0b3d1f}
.v3.mini.on .bico{background:#137a3c;border-color:#fff;color:#fff}
.v3.mini.on .grip{color:#d7efe0}
.v3.mini.on .sub{color:#ffffff}
.v3.mini.on .gbtn{border-color:#fff}
.v3.mini .sp{min-width:4px}

/* ---------- V4 Souris ---------- */
.v4.hud{width:344px;background:#161b24;color:#f2f5fa;border:1px solid #64708a;border-radius:16px;box-shadow:0 0 0 1px #000,0 10px 24px rgba(0,0,0,.45)}
.v4.hud.on{border:2px solid #34d27c}
.v4 .bar{height:42px;display:flex;align-items:center;gap:6px;padding:0 8px 0 10px;border-bottom:1px solid #263041}
.v4 .grip{width:14px;height:18px;color:#8794aa}
.v4 .brand{font-size:14px;font-weight:700;margin-left:2px}
.v4 .ver{font-size:12px;color:#b7c0ce}
.v4 .gbtn{height:28px;display:inline-flex;align-items:center;gap:5px;padding:0 9px;border-radius:7px;background:#222a38;border:1px solid #4b5870;color:#e3e9f2;font-size:12px;font-weight:600}
.v4 .gbtn .ic{width:11px;height:11px}
.v4 .body{display:grid;grid-template-columns:104px 1fr;gap:14px;padding:14px 14px 4px}
.v4 .figbox{display:flex;justify-content:center;padding-top:2px}
.v4 .name{font-size:14px;font-weight:700}
.v4 .state{display:flex;align-items:center;gap:6px;margin-top:6px;font-size:14px;font-weight:600;color:#dbe2ec}
.v4 .state .ic{width:11px;height:11px}
.v4 .state.on{color:#5fe3a0;font-weight:700}
.v4 .sub{font-size:12px;color:#b7c0ce;margin-top:2px;line-height:1.3}
.v4 .lab{font-size:12px;font-weight:600;color:#b7c0ce;margin-top:12px}
.v4 .keyrow{display:flex;align-items:center;gap:8px;margin-top:6px}
.v4 .kc.big{height:44px;min-width:58px;padding:0 12px 4px;border-radius:8px;font-size:20px;box-shadow:inset 0 -4px 0 #9aa4b1}
.v4 .kc.big.wait{box-shadow:none;padding-bottom:0}
.v4 .sbtn{height:30px;display:inline-flex;align-items:center;padding:0 11px;border-radius:7px;background:#283143;border:1px solid #6b7890;color:#f2f5fa;font-size:14px;font-weight:600}
.v4 .capt{margin-top:8px;padding:8px 10px;border-radius:8px;background:#10233a;border:1px solid #6cb8ff}
.v4 .capt .t1{font-size:14px;font-weight:600;color:#f3f5f8}
.v4 .capt .t2{font-size:12px;color:#b3d7ff;margin-top:2px}
.v4 .pbtn{margin-top:12px;height:36px;display:flex;align-items:center;justify-content:center;gap:8px;border-radius:8px;background:#f2f5fa;color:#111418;font-size:14px;font-weight:700}
.v4 .pbtn .ic{width:12px;height:12px}
.v4 .hint{margin-top:6px;font-size:12px;color:#b7c0ce}
.v4 .bottom{padding:0 14px 14px}
.v4 .seg{margin-top:6px;display:grid;grid-template-columns:repeat(3,1fr);gap:2px;height:34px;padding:2px;background:#11151c;border:1px solid #64708a;border-radius:8px}
.v4 .seg span{display:flex;align-items:center;justify-content:center;gap:5px;border-radius:6px;font-size:14px;font-weight:500;color:#e3e9f2}
.v4 .seg span.sel{background:#e3e9f2;color:#111418;font-weight:700}
.v4 .seg .ic{width:12px;height:12px}
.v4 .mwrap{padding:0 14px 4px}
.v4 .msg{margin-top:8px}
.v4 .tabs{display:grid;grid-template-columns:repeat(3,1fr);gap:6px;padding:10px 10px 0}
.v4 .tab{padding:8px;border-radius:10px;background:#1d2330;border:1px solid #34405a}
.v4 .tab.sel{background:#253047;border:2px solid #e3e9f2;padding:7px}
.v4 .tr{display:flex;align-items:center;gap:4px;height:24px}
.v4 .tr .mm{width:14px;height:20px}
.v4 .tr .ti{color:#7d8799}
.v4 .kc.sm{height:24px;min-width:30px;padding:0 6px 3px;font-size:12px}
.v4 .tn{margin-top:4px;font-size:12px;font-weight:700;line-height:1.25}
.v4 .ts{display:flex;align-items:center;gap:4px;margin-top:4px;font-size:12px;color:#c9d1dd}
.v4 .ts .ic{width:9px;height:9px}
.v4 .tab.soon .tn,.v4 .tab.soon .ts{color:#7d8799}
.v4.mini{width:auto;height:52px;display:flex;align-items:center;gap:10px;padding:0 8px 0 10px;border-radius:14px}
.v4.mini.on{padding:0 7px 0 9px}
.v4.mini .mm{width:18px;height:25px}
.v4 .mt{display:flex;flex-direction:column}
.v4 .ms{display:flex;align-items:center;gap:5px;font-size:14px;font-weight:700}
.v4 .ms .ic{width:10px;height:10px}
.v4.mini.on .ms{color:#5fe3a0}
.v4 .mb{font-size:12px;color:#b7c0ce}

/* ---------- V5 Colonnes ---------- */
.v5.hud{background:#121316;color:#f4f4f6;border:1px solid #5f6773;border-radius:14px;box-shadow:0 0 0 1px #000,0 10px 24px rgba(0,0,0,.45)}
.v5 .bar{height:36px;display:flex;align-items:center;gap:6px;padding:0 10px 0 8px}
.v5 .grip{width:14px;height:18px;color:#858c98}
.v5 .brand{font-size:14px;font-weight:700;margin-left:2px}
.v5 .ver{font-size:12px;color:#b6bac3}
.v5 .cols{display:flex;align-items:stretch;gap:8px;padding:0 8px}
.v5 .col{width:108px;display:flex;flex-direction:column;align-items:stretch;text-align:center;padding:10px 7px 8px;border-radius:10px;background:#1c1e23;border:1px solid #34373e}
.v5.single .col{width:188px;padding:10px 10px 10px}
.v5 .col.on{border:2px solid #34d27c;padding:9px 6px 7px}
.v5.single .col.on{padding:9px 9px 9px}
.v5 .kc.big{align-self:center;height:44px;min-width:60px;padding:0 12px 4px;border-radius:8px;font-size:18px;box-shadow:inset 0 -4px 0 #9aa4b1}
.v5 .kc.big.wait{box-shadow:none;padding-bottom:0}
.v5 .kc.big.off{box-shadow:inset 0 -4px 0 #1f2329}
.v5 .chg{align-self:center;margin-top:6px;height:26px;display:inline-flex;align-items:center;padding:0 9px;border-radius:6px;background:#26292f;border:1px solid #5a606b;color:#e1e3e8;font-size:12px;font-weight:600}
.v5 .chg.off{background:transparent;border-color:#33363d;color:#6f7682}
.v5 .ct1{margin-top:8px;font-size:14px;font-weight:600;color:#cfe6ff}
.v5 .ct2{margin-top:2px;font-size:12px;color:#9fc9f5}
.v5 .name{margin-top:10px;font-size:14px;font-weight:700;line-height:1.2}
.v5 .pill{margin-top:8px;height:28px;display:flex;align-items:center;justify-content:center;gap:6px;border-radius:14px;background:#2a2d34;border:1px solid #6b7280;color:#e9ebef;font-size:14px;font-weight:700}
.v5 .pill .ic{width:10px;height:10px}
.v5 .pill.on{background:#34d27c;border-color:#34d27c;color:#04140a}
.v5 .pill.soon{background:transparent;border:1px solid #5d6470;color:#9aa1ac;font-size:12px}
.v5 .sub{margin-top:4px;font-size:12px;font-weight:600;color:#5fe3a0}
.v5 .lab{margin-top:12px;text-align:left;font-size:12px;font-weight:600;color:#b6bac3}
.v5 .radios{margin-top:4px;display:flex;flex-direction:column;gap:2px}
.v5 .ra{height:28px;display:flex;align-items:center;gap:8px;padding:0 6px;border-radius:6px;font-size:14px;font-weight:500;color:#e9ebef}
.v5 .ra .dot{position:relative;flex:none;width:14px;height:14px;border-radius:7px;border:2px solid #9aa1ad}
.v5 .ra.sel{background:#2f333b;box-shadow:inset 0 0 0 1px #8b929e;font-weight:700}
.v5 .ra.sel .dot{border-color:#f4f4f6}
.v5 .ra.sel .dot::after{content:"";position:absolute;inset:2px;border-radius:50%;background:#f4f4f6}
.v5 .val{margin-top:4px;height:28px;display:flex;align-items:center;padding:0 8px;border-radius:6px;border:1px solid #33363d;font-size:14px;color:#7d8490;text-align:left}
.v5 .col.soon .name,.v5 .col.soon .lab{color:#7d8490}
.v5 .pbtn{margin-top:auto;height:34px;display:flex;align-items:center;justify-content:center;gap:6px;border-radius:8px;background:#f4f4f6;color:#111316;font-size:14px;font-weight:700}
.v5 .radios,.v5 .val{margin-bottom:10px}
.v5 .pbtn .ic{width:12px;height:12px}
.v5 .pbtn.off{background:#24262b;color:#6f7682}
.v5 .msg{position:relative;display:block;margin-top:8px;padding:8px 6px 8px 8px;text-align:left}
.v5 .msg .wi{position:absolute;left:8px;top:9px;width:15px;height:15px}
.v5 .msg .t1{padding-left:21px}
.v5 .msg .abtn{padding:0 8px}
.v5 .foot{display:flex;justify-content:flex-end;gap:4px;padding:6px 8px 8px}
.v5 .tbtn{height:28px;display:inline-flex;align-items:center;gap:5px;padding:0 8px;border-radius:6px;color:#d7dae0;font-size:12px;font-weight:600}
.v5 .tbtn .ic{width:11px;height:11px}
.v5.mini{display:flex;align-items:center;gap:6px;padding:6px 6px 6px 8px;border-radius:12px}
.v5.mini .kc{height:30px;min-width:38px;font-size:14px}
.v5.mini .kc.on{gap:5px;background:#34d27c;color:#04140a;box-shadow:inset 0 -3px 0 #1c8a4e}
.v5.mini .kc.on .ic{width:9px;height:9px}
.v5.mini .pill{margin-top:0;height:30px;padding:0 12px;border-radius:15px;font-size:12px}
.v5 .ibtn{width:30px;height:30px;border-radius:8px;display:inline-flex;align-items:center;justify-content:center;background:#26292f;border:1px solid #5a606b;color:#e1e3e8}
.v5 .ibtn .ic{width:13px;height:13px}
`;

// ---------------------------------------------------------------------------
// Page assembly
// ---------------------------------------------------------------------------

function tile(variant, view) {
  const label = view.k === 'f' && variant.fLabel ? variant.fLabel : view.label;
  return `<figure class="tile">
  <figcaption><b>${view.k}</b>${label}<span class="bgname">${BG_NAMES[view.bg]}</span></figcaption>
  <div class="stage"><svg class="scene" aria-hidden="true"><use href="#sc-${view.bg}"/></svg><div class="place">${variant.build(view.s)}</div></div>
</figure>`;
}

function section(variant) {
  return `<section class="variant" id="v${variant.n}">
  <div class="vhead"><div class="vnum">${variant.n}</div><div><h2>Variante ${variant.n} — ${variant.title}</h2><p>${variant.idea}</p></div></div>
  <div class="grid">${VIEWS.map((v) => tile(variant, v)).join('\n')}</div>
</section>`;
}

const html = `<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>MacroHUD · maquettes du HUD</title>
<style>${CSS}</style>
</head>
<body>
${SPRITE}
<header class="page-head">
  <h1>MacroHUD — cinq maquettes du HUD</h1>
  <p>Choisis une variante, de 1 à 5. Rien n’est codé : ce sont des images de travail. Taille réelle pour un affichage Windows à 100 %. Les fonds derrière le HUD sont simulés (bureau clair, jeu de nuit, jeu en plein jour) pour juger la lisibilité.</p>
  <ul class="legend">
    <li><b>a</b>arrêtée</li><li><b>b</b>active, bouton gauche enfoncé</li><li><b>c</b>capture d’un nouveau raccourci</li>
    <li><b>d</b>raccourci refusé</li><li><b>e</b>trois macros, dont deux «${NBSP}À venir${NBSP}»</li><li><b>f</b>forme repliée</li>
  </ul>
</header>
${VARIANTS.map(section).join('\n')}
<p class="foot-note">Police : Inter (libre, licence SIL Open Font License 1.1). Chaque HUD n’utilise que ce qu’egui sait dessiner : rectangles arrondis, couleurs pleines, contours, une ombre simple, du texte et des icônes simples. L’état n’est jamais montré par la couleur seule : il y a toujours un mot et une forme (■ arrêtée, ● active). Les deux macros «${NBSP}À venir${NBSP}» sont des exemples de remplissage, pas des décisions.</p>
</body>
</html>
`;

writeFileSync(OUT, html);
console.log(`écrit : ${OUT} (${Math.round(html.length / 1024)} Ko)`);
