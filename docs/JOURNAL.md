# Journal de bord

> Entrée la plus récente **en haut**. On ne modifie ni ne supprime une entrée passée :
> une erreur se corrige par une nouvelle entrée.

Gabarit :

```markdown
## AAAA-MM-JJ · <titre écrit pour l'utilisateur>

- **Session** : maître, programmation, visuel ou test
- **Branche et commit** :
- **Objectif** :
- **Réalisé** :
- **Décisions** :
- **Vérifications** : <commandes lancées et résultats>
- **Non vérifié sur le PC** : <points précis, reportés dans le plan>
- **Problèmes et dette** :
- **Prochaine étape** :
```

---

## 2026-11-20 · Le moteur « Maintenir le clic », les raccourcis et la livraison automatique

- **Session** : programmation
- **Branche et commit** : `claude/gifted-hypatia-g84kg1`, du commit `b8046e5` au commit vide
  `[ETAPE-FINIE programmation]`. L'exécutable à essayer est l'artefact `MacroHUD.0.1.<n>.exe` de
  l'exécution de la CI de ce dernier commit (onglet Actions, branche ci-dessus).
- **Objectif** : P1.1 à P1.3, tout le code non visuel.
- **Réalisé** :
  - Un seul exécutable `macrohud` (Rust, eframe/egui 0.37, rendu OpenGL), sans console ; une seule
    instance (la seconde le dit dans une boîte de message et se ferme).
  - « Maintenir le clic » : F8 enfonce le bouton choisi (Gauche par défaut, Droit, Milieu) par
    `SendInput` et le garde ; F8 à nouveau le relâche. Un vrai clic sur ce bouton arrête la macro
    (crochet `WH_MOUSE_LL` sur un fil dédié, événements injectés écartés par `LLMHF_INJECTED`) et
    le HUD passe à « arrêtée ». « Activer » dans le HUD enfonce le bouton 3 s plus tard (le temps de
    sortir la souris du HUD) ; F8 pressé la souris sur le HUD est refusé avec un message.
  - Bouton jamais coincé : rien n'est enfoncé tant que le crochet ne surveille pas ; changer de
    bouton pendant le maintien arrête la macro et relâche l'ancien ; « Quitter », Alt+F4, fermeture :
    relâche d'abord ; plantage Rust (tout fil) : le crochet de panique relâche, écrit
    `%APPDATA%\MacroHUD\plantages\plantage-<date>.txt`, affiche une boîte de message en français puis
    ferme ; plantage hors Rust (pilote) : un filtre d'exceptions relâche le bouton.
  - **Un arrêt brutal du processus ne peut pas être rattrapé** (Gestionnaire des tâches, fin de tâche
    sur le processus, coupure de courant) : si un bouton était maintenu, il reste enfoncé jusqu'à ce
    que l'utilisateur clique une fois ce bouton. Au lancement suivant, le HUD prévient (marqueur
    `session-en-cours` resté dans `%APPDATA%\MacroHUD`).
  - Raccourcis : un par macro (`RegisterHotKey`, sans répétition), actifs sans le focus ; « Changer »
    dans le HUD capture la touche ou la combinaison suivante (crochet clavier pendant la capture
    seulement ; Échap ou 10 s sans touche annulent ; les touches tapées n'atteignent aucune
    application, et un relâchement qui n'arrive jamais, après Win+L par exemple, n'est plus attendu
    au-delà de 10 s) ; noms en français (« F8 », « Ctrl+Alt+M »,
    « Arrêt défil. », « Win+Maj+S »). Refus expliqués, l'ancien raccourci restant actif : déjà pris
    par Windows ou un autre programme, par une autre macro, touche de frappe sans Ctrl/Alt/Win,
    F12 seule, Alt+F4, Alt+Tab, Alt+Espace, Ctrl+C/X/V/Z/Y/A/S, combinaison qui écrit un caractère
    avec AltGr sur le clavier en cours (Ctrl+Alt+E = « € » en AZERTY). Raccourci déjà pris au
    lancement : macro « indisponible » avec la raison, jusqu'au choix d'un autre raccourci.
  - Réglages `%APPDATA%\MacroHUD\reglages.json` (raccourcis, bouton, position en pixels d'écran,
    replié) ; absent : valeurs par défaut ; abîmé : valeurs par défaut, ancien fichier gardé sous
    `reglages.abime-<date>.json`, avertissement dans le HUD. Journal d'exécution `macrohud.log`.
  - Fenêtre : toujours au-dessus, sans bordure, déplaçable (le HUD envoie `StartDrag`), position
    retenue et ramenée dans l'écran si l'écran a changé ; taille donnée par `hud::window_size` ;
    redessinée dès qu'une macro bascule (le fil du moteur demande l'image) ; ne prend pas le focus
    quand une macro bascule ; repasse au premier plan des fenêtres « toujours au-dessus » à ce
    moment-là (sans prendre le focus), pour rester visible au-dessus d'un jeu en fenêtre sans bordure.
  - Contrat du HUD en types Rust : `src/contract.rs` (`HudState` reçu, `HudAction` renvoyées,
    `samples()` : un état par situation, pour les aperçus). HUD provisoire de debug :
    `src/hud/mod.rs`, widgets egui par défaut ; la session « visuel » le remplace en gardant les
    éléments décrits en tête du fichier (`setup`, `draw`, `window_size`, `TRANSPARENT`,
    `MESSAGE_DURATION`, `window_icon` ; une icône `src/hud/icone.ico` deviendrait celle de l'exécutable).
  - Options : `--version`, `--aide`, `--essai-plantage` (plante exprès quand on arrête un maintien).
  - CI (`.github/workflows/ci.yml`, modèle Massicot) : à chaque push sur n'importe quelle branche,
    tests Linux (fmt, clippy, `cargo test`), puis Windows (clippy, `cargo test` dont l'API Windows
    réelle, compilation release MSVC, essai de `--version`) et artefact `MacroHUD.<version>.exe` ;
    sur `main` seulement, Release `v<version>`. Version `0.1.<numéro d'exécution>`, inscrite dans le
    HUD, les propriétés du fichier et `--version`.
- **Décisions** (choix techniques, à numéroter par la session maître si elle les retient) :
  - rendu OpenGL (glow) plutôt que wgpu : exécutable plus petit, démarrage rapide, et seule voie pour
    une fenêtre transparente (coins arrondis) sous Windows si la maquette en veut une ;
  - le vrai clic qui arrête la macro est consommé : l'application voit un seul appui et un seul
    relâchement, ceux de la macro ; si l'utilisateur tenait déjà le bouton quand la macro démarre,
    MacroHUD le reprend (le lâcher ne relâche pas) ;
  - « Activer » depuis le HUD attend 3 s ; F8 avec la souris sur le HUD est refusé (l'appui
    tomberait sur le HUD lui-même) ;
  - changer de bouton pendant le maintien arrête la macro (relâche l'ancien, n'enfonce pas le nouveau) ;
  - refus des raccourcis qui casseraient la frappe ou Windows (liste ci-dessus) : à revoir si
    l'utilisateur veut une lettre seule ;
  - crochets bas niveau installés seulement quand ils servent (maintien, capture), sur un fil de
    priorité haute, réinstallés toutes les 5 s pendant un maintien (Windows retire sans prévenir un
    crochet qui a tardé) : la souris d'un jeu n'est pas ralentie le reste du temps ;
  - `src/hud/` fournit aussi la taille de la fenêtre, sa transparence, la durée des messages et l'icône.
- **Vérifications** :
  - `cargo fmt --check` : OK. `cargo clippy --all-targets -- -D warnings` : OK.
    `cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings` (depuis Linux) : OK.
  - `cargo test` : 79 tests OK sous Linux (moteur avec un faux Windows : ordre surveillance → appui
    et relâche → fin de surveillance, vrai clic, changement de bouton, délai du HUD, conflits de
    raccourcis, indisponibilité, capture ; garde du clic ; capture clavier ; réglages abîmés ; HUD
    dessiné sans fenêtre pour chaque état de `samples()` ; rapport de plantage).
  - CI, exécution n° 1 (commit `b8046e5`) : tout vert. Linux : fmt, clippy, tests. Windows
    (windows-latest, MSVC) : clippy ; 83 tests, dont 5 propres à Windows avec la vraie API (un
    raccourci déjà enregistré est bien refusé comme « pris », le fil des crochets installe et retire
    les crochets souris et clavier, constantes identiques à celles de Windows) ; compilation release
    (exécutable de 6,1 Mo) ; `MacroHUD.0.1.1.exe --version` répond « MacroHUD 0.1.1 » ; artefact
    déposé ; publication sautée (pas `main`). Les exécutions suivantes de la branche refont tout.
  - HUD lancé sous Linux (écran virtuel et clics simulés par `xdotool`, souris réelle non touchée) :
    « Activer » (message, état « active », appui simulé 3 s plus tard), « Arrêter », « Droit »
    (réglages écrits), « Changer » puis « Annuler » (« Raccourci inchangé : F8. »), « Replier »
    (fenêtre réduite, réglage écrit), « Quitter » pendant un maintien (relâché avant de sortir),
    relance (réglages repris), arrêt brutal puis relance (avertissement affiché), `--essai-plantage`
    (bouton relâché, rapport écrit, message, sortie).
- **Non vérifié sur le PC** (je n'ai pas Windows ; R1 à R16 du plan) : tout le comportement réel :
  appui et relâche reçus par Paint et par un jeu, vrai clic qui arrête, raccourci global sans le
  focus, refus d'un raccourci pris par Windows, AltGr, HUD au-dessus d'un jeu en fenêtre sans
  bordure, position retenue (pixels d'écran, plusieurs écrans), boîte de message de plantage,
  seconde instance, propriétés du fichier, accueil de SmartScreen et de l'antivirus.
- **Problèmes et dette** :
  - un jeu ou une application lancé en administrateur ne reçoit pas les clics injectés si MacroHUD
    ne l'est pas (protection de Windows) ; certains anti-triche refusent les clics injectés ;
  - en plein écran exclusif, aucun HUD ne peut s'afficher par-dessus (plein écran fenêtré seulement) ;
  - sans pilote OpenGL 2 (machine virtuelle sans carte graphique), la fenêtre ne s'ouvre pas : message
    clair dans ce cas ;
  - sous Linux, MacroHUD se lance (pour la session « visuel ») mais souris et clavier sont simulés ;
    dans un conteneur sans écran : `apt-get install -y libxkbcommon-x11-0 xvfb`, `Xvfb :97 &`, puis
    `DISPLAY=:97 LIBGL_ALWAYS_SOFTWARE=1 cargo run` (et `import -window root capture.png` pour voir).
- **Prochaine étape** : test sur le PC (P1.6) avec l'artefact de la CI ; maquettes (session « visuel »).

---

## 2026-11-20 · Le HUD choisi (variante 2) dessiné en egui, en attente du moteur

- **Session** : visuel
- **Branche et commit** : `claude/gifted-turing-0izrcw`, commit « HUD variante 2 en egui, avant
  branchement » (après fusion de `origin/main` pour D-12).
- **Objectif** : étape 2, premier temps (demande de la session maître) : coder le HUD de la
  variante 2 dans `src/hud/` sans attendre la session « programmation », et en produire des PNG du
  vrai rendu egui.
- **Réalisé** : `src/hud/` (egui 0.37) : types de vue miroirs du contrat (`view.rs`), couleurs et
  mesures de la maquette (`style.rs`), police Inter embarquée avec sa licence OFL (`police/`, le
  fichier variable exact des maquettes, 120 Ko ; graisses par l'axe `wght`), icônes en formes
  simples, composants (ligne de macro, touche, sélecteur Gauche/Droit/Milieu, zone « Appuie sur une
  touche… », messages, pastille repliée), `Hud::size` et `fit_window` pour la fenêtre. Rendu
  logiciel en PNG pour les tests (`render.rs`, sans carte graphique). Crate d'essai
  `maquettes/egui/` (inclut `src/hud/` tel quel) : `rendus/variante-2-egui.png` reprend la planche de
  la maquette avec le vrai rendu ; `rendus/hors-maquette-egui.png` montre trois états que la
  maquette n'avait pas (macro indisponible, message d'information, d'erreur), à valider.
- **Décisions** : aucune décision de l'utilisateur. Choix de présentation, à confirmer par la
  session maître : réglages d'une macro fermés au lancement (la flèche les ouvre ; ils s'ouvrent
  seuls pendant la capture d'un raccourci ou quand un message concerne la macro) ; pastille repliée
  à plusieurs macros : la première active, sinon la première disponible ; macro indisponible : sa
  raison en entier dans un encadré sous sa ligne.
- **Vérifications** : dans `maquettes/egui/` : `cargo test` (6 tests verts : chaque vue a la taille
  exacte de la maquette ; ligne active verte au pixel près ; de vrais clics souris envoyés à egui
  donnent Activer, Arrêter, bouton Droit, Changer, Annuler, Replier, Déplier, Quitter ; une macro
  « À venir » ne réagit pas ; la fenêtre n'est redimensionnée qu'une fois par changement),
  `cargo fmt --check` et `cargo clippy --all-targets -- -D warnings` sans remarque. Planche egui
  comparée à la maquette agrandie trois fois : mêmes positions, écarts limités au lissage du texte.
- **Non vérifié sur le PC** : fenêtre réelle sous Windows (transparence des coins arrondis, mise à
  l'échelle 125/150 %, rendu GPU d'eframe au lieu du rendu logiciel des tests).
- **Problèmes et dette** : à trancher avec la session maître, sans toucher au moteur :
  1. Contrat, message : le HUD affiche un titre (« Raccourci refusé ») et une explication, et doit
     savoir quelle macro est concernée pour proposer « Choisir une autre touche » ; le contrat ne
     prévoit qu'un texte et un niveau.
  2. Contrat, macros « À venir » : le HUD montre un petit dessin par sorte de macro (souris, clic
     répété, touche) ; l'intégration devra dire quelle sorte est chaque macro.
  3. Fenêtre : pas d'ombre dans la vraie fenêtre (sa marge transparente avalerait les clics autour
     du HUD) ; coins arrondis par une fenêtre transparente (rayon 9) ; suggestion : que cliquer sur
     le HUD ne retire pas le focus au jeu (style Windows « no activate »).
  4. Version d'egui : codé pour egui 0.37 ; à aligner sur la version que choisira la session
     « programmation ».
- **Prochaine étape** : recevoir le nom de la branche « programmation », la fusionner avec
  `origin/main`, brancher `src/hud/` sur ses types du contrat, régler la fenêtre, garder le test de
  rendu PNG dans le crate principal.

---

## 2026-11-20 · Cinq maquettes du HUD à choisir

- **Session** : visuel
- **Branche et commit** : `claude/gifted-turing-0izrcw`, commit « Maquettes du HUD : cinq variantes
  à choisir », puis le marqueur vide `[ETAPE-FINIE visuel-maquettes]`.
- **Objectif** : P1.4, cinq maquettes du HUD pour que l'utilisateur en choisisse une.
- **Réalisé** : `maquettes/index.html` (page autonome, police embarquée, aucune ressource
  externe) ; `maquettes/variante-1.png` à `variante-5.png` (1600 px de large, six vues a à f
  chacune, sur fonds simulés) ; `maquettes/LISEZMOI.md` (idée, force, faiblesse, recommandation).
  Variantes : 1 Cartes, 2 Liste compacte, 3 Grand état (thème clair), 4 Souris, 5 Colonnes.
  Recommandation : la 2. Outils dans `maquettes/outils/` : `generer.mjs` écrit la page,
  `rendre.mjs` produit les PNG avec Playwright et contrôle contraste et débordements ; police
  Inter (`polices/`, licence OFL jointe).
- **Décisions** : aucune décision de l'utilisateur. À acter par la session maître si une variante
  est retenue : police Inter (SIL OFL 1.1) embarquée dans l'exécutable, car la police par défaut
  d'egui est fine et sans gras. Les deux macros « À venir » (Clic répété F9, Touche maintenue F10)
  ne sont que des exemples de remplissage.
- **Vérifications** : `node maquettes/outils/generer.mjs` puis `node maquettes/outils/rendre.mjs` :
  cinq images produites ; contraste de chaque texte de chaque HUD mesuré, 5,4:1 au minimum (texte
  grisé « À venir » 3,3:1 au minimum) ; aucun texte coupé ni sorti du panneau (contrôle vérifié
  sur une copie volontairement cassée, qu'il détecte) ; images en niveaux de gris relues : arrêtée
  et active se distinguent sans couleur ; page sans défilement horizontal à 390 px (téléphone).
- **Non vérifié sur le PC** : rien ne tourne encore ; le rendu du texte par egui diffère un peu de
  celui du navigateur, il se verra à l'étape 2.
- **Problèmes et dette** : aucun.
- **Prochaine étape** : l'utilisateur choisit une variante (ou un mélange) ; la session maître
  transmet ce choix et le nom de la branche « programmation » ; étape 2 : coder le HUD choisi dans
  `src/hud/`.

---

## 2026-11-20 · Démarrage de MacroHUD

- **Session** : maître
- **Branche et commit** : `main`, premier commit (mémoire du projet, sans code).
- **Objectif** : cadrer MacroHUD, poser la mémoire, lancer les sessions de travail.
- **Réalisé** : cadrage (D-01 à D-11), dépôt public, mémoire du projet, contrat du HUD
  (`docs/CONTRAT-HUD.md`), miroir Drive.
- **Décisions** : D-01 à D-11.
- **Vérifications** : aucune (pas encore de code).
- **Non vérifié sur le PC** : R1 à R4.
- **Problèmes et dette** : aucun.
- **Prochaine étape** : session cloud « programmation » (moteur, raccourcis, CI) et session cloud
  « visuel » (cinq maquettes du HUD) en parallèle ; choix d'une maquette par l'utilisateur ; test
  sur le PC par la session locale « test ».
