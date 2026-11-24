# Maquettes du HUD : cinq variantes à choisir

Ouvre `index.html` (aucune connexion nécessaire) ou regarde les images `variante-1.png` à
`variante-5.png`. Chaque image montre la variante en six vues, à taille réelle pour un affichage
Windows à 100 %, sur des fonds simulés (bureau clair, jeu de nuit, jeu en plein jour) :
**a** arrêtée · **b** active, bouton gauche enfoncé · **c** capture d'un nouveau raccourci ·
**d** raccourci refusé · **e** trois macros, dont deux « À venir » · **f** forme repliée.

## Les cinq variantes

| N° | Nom | Idée | Force | Faiblesse |
|---|---|---|---|---|
| 1 | Cartes | Panneau vertical, une carte complète par macro ; active : la carte se borde de vert | Tout est visible et nommé, sans aucun clic | Haut (330 px pour une macro, 500 px à trois) ; chaque vraie macro ajoutera une carte complète |
| 2 | Liste compacte | Une ligne par macro (bouton ▶, état, touche) ; les réglages s'ouvrent sous la ligne | Le plus petit (288 × 84 px réglages fermés) ; active : toute la ligne verte, visible du coin de l'œil ; une macro de plus = une ligne de plus | Le bouton ▶ n'a pas de libellé écrit ; bouton maintenu et raccourci demandent un clic sur la flèche |
| 3 | Grand état | Thème clair ; grand bandeau d'état avec le bouton Activer dedans ; repliée : le bandeau seul | L'état se lit de loin, même replié | Le panneau blanc éblouit dans un jeu sombre et se détache moins sur un ciel clair ; haut (455 px à trois macros) |
| 4 | Souris | Une souris dessinée : on choisit le bouton sur le dessin, le bouton enfoncé s'allume en vert avec une flèche | Le choix du bouton se comprend sans lire | Le plus large (344 px) ; le dessin ne vaut que pour les macros de souris, pas pour une macro de clavier |
| 5 | Colonnes | Une colonne par macro, coiffée de sa grande touche de raccourci ; repliée : une rangée de touches | Le raccourci de chaque macro saute aux yeux ; la forme repliée à plusieurs macros est la plus parlante | Haut et étroit pour une macro (≈ 400 px) ; s'élargit à chaque macro (au-delà de trois, trop large) |

## Ma recommandation : la 2, « Liste compacte »

- C'est un HUD posé sur un jeu : la 2 cache le moins d'écran (une ligne de 50 px par macro) et se
  replie en une pastille de 34 px.
- L'état actif est le plus visible pour la place prise : toute la ligne passe au vert, avec le mot
  « Active » et le rond ●, donc lisible aussi sans la couleur.
- Elle grandit proprement : chaque nouvelle macro ajoute une ligne, avec sa touche alignée à droite
  ; on lit d'un coup « quelle touche fait quoi ».
- Ce qu'on change rarement (bouton maintenu, raccourci) est à un clic, ce qu'on regarde souvent
  (l'état) est toujours là.

Deuxième choix : la 1 « Cartes », si tu préfères tout voir sans jamais ouvrir de réglages. Les
mélanges sont possibles (par exemple la 2 avec la forme repliée « rangée de touches » de la 5), et
les couleurs claires ou sombres passent d'une variante à l'autre.

## Ce qui vaut pour les cinq

- Police **Inter** (libre, licence SIL Open Font License 1.1), embarquée dans la page et, plus
  tard, dans l'exécutable : la police par défaut d'egui est trop fine et n'a pas de gras.
- Tout est dessinable en egui : rectangles arrondis, couleurs pleines, contours, une ombre simple,
  texte et icônes simples (le dessin de souris de la 4 n'est fait que de formes simples).
- L'état n'est jamais dit par la couleur seule : toujours un mot et une forme (■ arrêtée, ● active,
  « À venir » écrit). Vérifié en regardant les cinq images passées en niveaux de gris.
- Contraste mesuré automatiquement sur chaque texte de chaque HUD : 5,4:1 au minimum (7,5:1 pour
  les variantes sombres) ; texte grisé « À venir » : 3,3:1 au minimum, lisible. Le bord du panneau
  (gris moyen cerclé de noir) se voit sur fond blanc comme sur fond noir.
- Boutons de 24 à 38 px de haut, jamais sous le minimum de 24 px.
- Les deux macros « À venir » (Clic répété, Touche maintenue) sont des exemples pour montrer la
  place prise : ni leur nom ni leurs options ne sont des décisions.
- Repliée, le HUD ne montre pas sa version : elle réapparaît en le dépliant.

## Refaire la page et les images

```bash
node maquettes/outils/generer.mjs   # écrit index.html
node maquettes/outils/rendre.mjs    # écrit les PNG et contrôle contraste et débordements
```

## Rendu réel du HUD codé (variante 2, D-12)

Le HUD retenu est codé dans `src/hud/`. Ses images ne sont pas des maquettes : c'est le vrai
code du HUD, branché sur le contrat (`src/contract.rs`), rendu sans écran par les tests.

- `rendu-egui/variante-2-egui.png` : les six vues de la maquette, même mise en page et mêmes fonds
  que `variante-2.png`, pour les comparer d'un coup d'œil.
- `rendu-egui/situations-egui.png` : chaque situation de `contract::samples()`, plus le délai de 3 s
  après « Activer » et F8 refusé avec la souris sur le HUD.
- `rendu-egui/vue-*.png` (100 % et 200 %) et `rendu-egui/situation-*.png` : chaque image seule.

```bash
MACROHUD_RENDUS=maquettes/rendu-egui cargo test hud::   # refait toutes ces images
```

Sans `MACROHUD_RENDUS`, `cargo test` écrit les vues seules dans `target/rendus-hud/`. Les fonds des
planches (`fonds/`) viennent de la page des maquettes : `node maquettes/outils/rendre.mjs --fonds maquettes/fonds`.

## Aperçu de « Multi-clic » (tâche P2.2)

`multi-clic/apercu.html` : une page sans script, lisible à 400 px de large, qui s'ouvre seule (ses
images y sont intégrées). En haut, le HUD d'aujourd'hui ; puis le HUD à deux macros (« Maintenir le
clic » F8, « Multi-clic » F9) en trois pistes pour régler la rapidité : A, un curseur de 1 à 15 ;
B, des boutons − et + ; C, trois préréglages et un réglage fin. Trois vues par piste : réglages
ouverts, macro arrêtée ; active ; active avec la souris sur le HUD (clics en pause). En bas, les
questions pour l'utilisateur. Ce sont des maquettes : rien n'est codé dans `src/hud/` avant son choix.

- `multi-clic/aujourdhui-*@2x.png` : copies de `rendu-egui/vue-a@2x.png` et `vue-b@2x.png`, le HUD
  actuel rendu par son propre test.
- `multi-clic/a-*`, `b-*`, `c-*@2x.png` : les neuf vues, dessinées à 200 % avec egui par
  `multi-clic/rendu/`, un petit programme à part (hors de la compilation de MacroHUD et de sa CI) qui
  reprend tels quels les fichiers du HUD (`style.rs`, `text.rs`, `icons.rs`, `parts.rs`) et le
  contrat (`src/contract.rs`) ; seule la mise en page à deux macros et le réglage de la rapidité sont
  propres à la maquette. « Multi-clic » n'étant pas encore dans le contrat (P2.1), ses données y sont
  décrites d'après `docs/CONTRAT-HUD.md`.

```bash
MACROHUD_RENDUS=maquettes/rendu-egui cargo test hud::                         # le HUD actuel, d'abord
cargo run --release --manifest-path maquettes/multi-clic/rendu/Cargo.toml     # les neuf vues et les pages
cargo test --release --manifest-path maquettes/multi-clic/rendu/Cargo.toml    # aucune ligne d'état coupée
```

Le texte des pages se modifie dans `multi-clic/rendu/apercu.modele.html` et
`resultat.modele.html`, puis la commande ci-dessus refait `apercu.html` et `resultat.html`.

## Résultat de « Multi-clic » (tâche P2.3)

La piste A choisie (D-26) est codée dans `src/hud/`. `multi-clic/resultat.html` (sans script, images
intégrées) place le vrai HUD à côté des trois vues de la piste A, avec l'écart mesuré pixel à pixel
sous chaque paire, puis le vrai HUD dans chaque situation du contrat où « Multi-clic » paraît.

- `rendu-egui/multi-1-reglages`, `multi-2-active`, `multi-3-en-pause` (100 % et 200 %) : les trois
  vues de l'aperçu, dessinées par le vrai HUD (la flèche de la souris de la troisième est ajoutée
  par le test, comme dans l'aperçu) ; `multi-4-replie-en-pause@2x.png` : la pastille pendant une
  pause.
- `rendu-egui/situation-14` à `19` : les situations du contrat avec « Multi-clic » (200 % en plus).

Les mêmes commandes que pour l'aperçu refont ces images et la page.

## Macro externe dans le HUD (tâche P4.5)

La ligne d'une macro externe est codée dans `src/hud/` d'après l'aperçu validé
`macro-externe/apercu.html` (D-37). `macro-externe/resultat.html` (sans script, images intégrées)
place chaque vue de l'aperçu à côté du vrai HUD et dit ce qui diffère.

- `rendu-egui/externe-*@2x.png` : les vues de l'aperçu dessinées par le vrai HUD, avec ses textes
  d'exemple (macro « Exemple »), puis trois cas que l'aperçu ne montre pas (souris sur le HUD, avant
  le premier état, sans raccourci) ; `rendu-egui/situation-20` à `32` : les mêmes situations du
  contrat, à 100 %.
- `macro-externe/apercu-1@2x.png` à `apercu-8@2x.png` : les vues de l'aperçu photographiées à 200 %.

```bash
MACROHUD_RENDUS=maquettes/rendu-egui cargo test hud::                         # le vrai HUD
node maquettes/outils/apercu-externe.mjs                                      # l'aperçu, photographié
cargo run --release --manifest-path maquettes/multi-clic/rendu/Cargo.toml     # les pages
```
