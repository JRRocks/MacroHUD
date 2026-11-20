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

## 2026-11-20 · Tests sur le PC et première livraison : MacroHUD 0.1.10

- **Session** : maître
- **Branche et commit** : `main` avancé de `9ee97a2` à `f9f8d4c` (avance rapide poussée du PC, pas de
  fusion par GitHub), sommet de `claude/gifted-hypatia-g84kg1`, qui contient le HUD de
  `claude/gifted-turing-0izrcw` ; CI n° 10 sur `main` → Release `v0.1.10`.
- **Objectif** : faire vérifier sur le PC le travail des deux sessions cloud, puis livrer la première
  version.
- **Réalisé** :
  - Avant chaque passage, halte confirmée de toutes les autres sessions Claude du PC (D-14) ; reprise
    envoyée après chaque passage.
  - Passage 1 (exécutable 0.1.5 : moteur, HUD provisoire) : 12 points OK, 1 échec (capture d'un
    raccourci quand le HUD a le focus), 1 non vérifiable (vrai clic physique) ; deux réserves (clics sur
    le HUD sans effet pendant un maintien droit ou milieu ; ligne « active » pendant l'attente de 3 s).
    Corrigé par la session « programmation » (correctif 1).
  - Passage 2 (exécutable 0.1.9, `f9f8d4c` : moteur corrigé, HUD variante 2) : 19 points OK, 0 échec,
    1 non vérifiable (vrai clic) ; un défaut nouveau (P1.8) et des écarts à la maquette (P1.9).
    Rapports et captures gardés sur le PC, hors du dépôt (`essais-prives/`).
  - L'utilisateur choisit de publier tel quel et de corriger dans la version suivante (D-15). Écarts à
    la maquette non retenus (D-16) ; choix des sessions cloud retenus (D-17). Contrat du HUD complété :
    un message dit quelle macro il concerne ; sorte « raccourci refusé ».
  - Contrôle « dépôt public » de tout ce qui partait dans `main` : identités `noreply` seulement, rien
    à signaler.
  - Release `v0.1.10` (`MacroHUD.0.1.10.exe`) ; miroir Drive mis à jour.
  - Corrections envoyées : P1.8 et la partie contrat de P1.9 à « programmation », P1.9 à « visuel ».
- **Décisions** : D-15, D-16, D-17.
- **Vérifications** : passages 1 et 2 par la session « test » sur le PC (exécutable de la CI, souris et
  clavier simulés, fenêtre d'essai qui compte les messages de la souris) ; CI n° 10 verte, l'exécutable
  publié répond « MacroHUD 0.1.10 » dans la CI.
- **Non vérifié sur le PC** : le vrai clic qui arrête la macro (R3) ; un jeu (R4, R17) ; l'échelle
  d'affichage à 125 % et 150 % (R20) ; Win+E et AltGr+E (R6, R7).
- **Problèmes et dette** : P1.8 (l'application ne reçoit pas le relâchement après un arrêt par le
  HUD) ; P1.9 (écarts à la maquette).
- **Prochaine étape** : correctifs des deux sessions cloud, passage 3 sur le PC (avec halte), accord de
  l'utilisateur, livraison suivante.

---

## 2026-11-20 · Correctif 1 : capture avec le HUD au premier plan, clics pendant un maintien, décompte

- **Session** : programmation
- **Branche et commit** : `claude/gifted-hypatia-g84kg1`, avancée rapide sur la branche intégrée
  `claude/gifted-turing-0izrcw` (`ba0452a`, HUD variante 2), puis le correctif, puis le commit vide
  `[ETAPE-FINIE programmation-correctif-1]`. L'exécutable à essayer est l'artefact de la CI de ce
  dernier commit.
- **Objectif** : les trois corrections demandées après le test sur le PC de 0.1.5 (12 points OK,
  1 échec, 1 non vérifiable).
- **Réalisé** :
  1. **Capture d'un raccourci avec le HUD au premier plan** (l'échec : 7 touches perdues sur 7).
     Je n'ai pas Windows pour voir pourquoi le crochet clavier global (`WH_KEYBOARD_LL`) ne reçoit
     pas ces touches-là ; plutôt que de deviner, la capture écoute maintenant de trois façons, et la
     première réponse gagne :
     - le crochet global, comme avant (touches tapées dans n'importe quelle fenêtre) ;
     - un crochet clavier posé sur le seul fil du HUD (`WH_KEYBOARD`), installé au clic sur
       « Changer » et retiré à la fin de la capture : il reçoit les touches qui arrivent au HUD
       quand il a le focus, et les garde pour lui (le HUD ne les voit pas) ;
     - un raccourci déjà enregistré pressé pendant une capture (F8, ou celui d'une autre macro) :
       Windows le livre comme raccourci et non comme touche ; c'est désormais la réponse de la
       capture (« Raccourci inchangé : F8. »), et non plus un ordre d'activer la macro.
     Pour comprendre le prochain essai : `macrohud.log` dit, à chaque capture, par quelle voie la
     touche est arrivée et combien d'événements clavier chaque crochet a vus.
     Limite connue : avec le HUD au premier plan, une combinaison que Windows traite lui-même avant
     toute fenêtre (Alt+Tab, Win+…) ne peut être vue que par le crochet global.
  2. **Clics sur le HUD pendant un maintien du bouton droit ou du milieu.** Cause : une règle de
     Windows. La fenêtre qui reçoit l'appui (Paint, le jeu) « capture » la souris tant que le bouton
     est tenu : tous les clics vont alors à elle, même au-dessus du HUD (ni winit ni egui n'y sont
     pour rien ; avec le bouton gauche, le problème ne se voyait pas car le clic gauche arrête déjà
     le maintien). Correction : pendant un maintien, la surveillance de la souris intercepte un clic
     sur le HUD (n'importe quel bouton, de la main ou d'un autre programme comme le test), arrête
     tous les maintiens et garde ce clic pour elle : il n'arrive pas au jeu. Message : « « Maintenir
     le clic » arrêtée par ton clic sur le HUD : clique à nouveau pour agir sur le HUD. » Le clic
     suivant agit normalement sur le HUD. F8 et le vrai clic sur le bouton tenu arrêtent toujours la
     macro. Les événements injectés par MacroHUD lui-même sont reconnus à leur signature et ignorés.
  3. **Décompte de « Activer ».** Pendant les 3 s, la macro reste « arrêtée » (rien n'est enfoncé) ;
     le message ponctuel du contrat, niveau info, décompte : « Activation dans 3 s : place la souris
     là où le clic doit rester enfoncé. », puis 2 s, puis 1 s. À l'appui, ou si le décompte est
     annulé (F8, « Arrêter »), le moteur retire ce message (le contrat ne change pas : le message
     disparaît simplement du HUD). Changer de bouton pendant le décompte le laisse continuer avec le
     nouveau bouton.
  - Messages du moteur passés au tutoiement, comme le HUD (« Appuie sur une touche… ») et le
    message demandé (« place la souris ») : « Sors la souris du HUD… », « choisis-en un autre »,
    « clique-le une fois »… Les deux textes d'exemple écrits dans `src/hud/render.rs` (rendus PNG de
    la session « visuel ») gardent l'ancienne formulation : à mettre à jour par elle si voulu.
  - Aucun fichier de `src/hud/` modifié.
- **Décisions** (choix techniques, à numéroter par la session maître si elle les retient) :
  - pendant un maintien, un clic sur le HUD arrête d'abord tous les maintiens et n'agit pas sur le
    HUD (seule façon de garder le HUD utilisable, et d'éviter que ce clic tombe dans le jeu) ;
  - pendant une capture, un raccourci enregistré compte comme la touche capturée ;
  - le moteur peut retirer son message (`Notice` sans message) ; tutoiement de tous les messages.
- **Vérifications** :
  - `cargo fmt --check` : OK. `cargo clippy --all-targets -- -D warnings` sous Linux et pour la cible
    `x86_64-pc-windows-msvc` : OK. `cargo test` : 100 tests OK sous Linux, dont les nouveaux :
    décompte 3, 2, 1 puis appui et retrait du message ; tic en retard ; annulation par F8 et par
    « Arrêter » ; changement de bouton pendant le décompte ; clic sur le HUD qui arrête tous les
    maintiens (de la main et d'un autre programme) ; clic d'un autre programme ailleurs ignoré ;
    raccourci enregistré pendant une capture ; première réponse de capture gagnante ; décodage des
    messages clavier du fil du HUD ; retrait d'un message côté HUD.
  - CI, exécution n° 8 (commit `0d2990e`, le code de ce correctif) : tout vert. Windows : clippy ;
    106 tests, dont l'installation et le retrait réels du crochet clavier du fil du HUD ; compilation
    release ; `MacroHUD.0.1.8.exe --version` répond « MacroHUD 0.1.8 ». Le commit vide qui suit
    refait tout et donne l'exécutable à essayer.
  - Le vrai programme sous Linux (écran virtuel, clics simulés, HUD variante 2) : ▶ → ligne
    « Arrêtée · bouton gauche » et encadré « Activation dans 2 s : place la souris… » pendant le
    décompte, puis ligne verte « Active » et encadré disparu à l'appui ; « Quitter » → bouton
    relâché, fermeture normale.
- **Non vérifié sur le PC** : R21 (capture avec le HUD au premier plan), R22 (clic sur le HUD pendant
  un maintien droit ou milieu), R23 et R8, R18 mis à jour (décompte), R10 mis à jour (« Quitter »
  pendant un maintien : premier clic = arrêt).
- **Problèmes et dette** : la cause exacte du crochet global muet quand le HUD a le focus reste à
  établir sur le PC (voir `macrohud.log`) ; avec le HUD au premier plan, Alt+Tab et les raccourcis
  Win+… ne peuvent pas être capturés si ce crochet ne les voit pas.
- **Prochaine étape** : nouveau test sur le PC (R8, R10, R18, R21 à R23), puis fusion par la session
  maître.

---

## 2026-11-20 · Le HUD choisi (variante 2) branché sur le moteur

- **Session** : visuel
- **Branche et commit** : `claude/gifted-turing-0izrcw` ; fusions de `claude/gifted-hypatia-g84kg1`
  (programmation, `b18e4fb`) et de `main` (D-13, D-14), puis commit « HUD variante 2 branché sur le
  contrat », puis le marqueur vide `[ETAPE-FINIE visuel-hud]`.
- **Objectif** : étape 2, second temps : brancher le HUD de la variante 2 sur les types du contrat,
  régler la fenêtre, garder le test de rendu PNG dans le crate principal.
- **Réalisé** :
  - `src/hud/` remplace le HUD provisoire en gardant son interface (`setup`, `draw`, `window_size`,
    `TRANSPARENT`, `MESSAGE_DURATION`, `window_icon`) : il dessine depuis `HudState` et renvoie les
    `HudAction` du contrat (Start, Stop, SetOptions, StartCapture, CancelCapture, SetCollapsed,
    StartDrag, Quit). Seule mémoire entre deux images : quelle macro a ses réglages dépliés.
  - Le texte est mesuré par des polices à part : `window_size` (sans contexte egui) et `draw`
    calculent exactement la même mise en page.
  - Messages du contrat (un texte et un niveau) : encadré avec un titre selon le niveau
    (« Information », « Attention », « Erreur ») et le texte dessous ; dans les réglages ouverts,
    sinon en bas du HUD, et sous la pastille quand le HUD est replié.
  - Les deux cas signalés par la session maître : « Activer » → la ligne passe au vert et l'encadré
    bleu du moteur annonce l'appui dans 3 s ; F8 avec la souris sur le HUD → encadré « Attention »,
    visible même replié.
  - Fenêtre : `TRANSPARENT = true` (coins arrondis, pastille), taille du HUD avec son anneau noir
    d'1 px, sans ombre ; pas d'icône propre (elle demanderait ses maquettes).
  - Tests de rendu dans le crate principal (`src/hud/render.rs`) ; le crate d'essai
    `maquettes/egui/` est supprimé, ses fonds passent dans `maquettes/fonds/` ; rendus du vrai HUD
    dans `maquettes/rendu-egui/` (planche des six vues de la maquette, planche de toutes les
    situations de `samples()` plus les deux cas ci-dessus).
  - Hors de `src/hud/`, deux retouches minimes, signalées ici : `src/app.rs` reçoit `clear_color`
    transparent (eframe efface par défaut en gris sombre semi-opaque : le HUD aurait eu des coins
    gris malgré `TRANSPARENT`) ; `Cargo.toml` reçoit `png` en dépendance de test.
- **Décisions** (choix de présentation, à confirmer par la session maître) : titre d'un message =
  son niveau en mots ; « Active · bouton gauche » au lieu de « Active · gauche enfoncé » de la
  maquette (vrai aussi pendant les 3 s avant l'appui) ; « bouton milieu » (« du milieu » serait coupé) ;
  raccourci très long coupé par « … » sur la ligne ; version trop longue (« 0.1.0-dev » d'une
  compilation locale) coupée avant « Replier » ; plus de bouton « Choisir une autre touche » (le
  contrat ne dit pas quelle macro un message concerne) ; macro « À venir » : dessin selon sa sorte,
  déduite de ses options.
- **Vérifications** :
  - `cargo fmt --check` ; `cargo clippy --all-targets --locked -- -D warnings` sous Linux et pour la
    cible `x86_64-pc-windows-msvc` : sans remarque. `cargo test --locked` : 89 tests verts, dont
    11 du HUD (tailles identiques à la maquette ; vrais clics envoyés à egui → actions du contrat ;
    glisser la barre ou la pastille → StartDrag ; macros à venir inertes ; version longue et lignes
    d'état jamais coupées ; message sous la pastille) et celui de la session « programmation » qui
    dessine chaque situation de `samples()`.
  - Le vrai programme (`cargo run`, fenêtre eframe, OpenGL logiciel) sous un écran virtuel Xvfb,
    clics simulés par xdotool : HUD en (0, 0), 290 × 86 ; flèche → réglages (290 × 190) ;
    « Activer » → ligne verte et encadré des 3 s, appui (simulé) 3 s plus tard d'après le journal ;
    « Milieu » enregistré dans les réglages ; « Changer » → zone de capture (290 × 242),
    « Annuler » → « Raccourci inchangé : F8. » ; « Replier » avec ce message → pastille et encadré
    dessous (290 × 105) ; « Déplier » ; « Quitter » pendant un maintien → bouton relâché puis fermeture
    normale. Cet essai a montré « 0.1.0-dev » qui touchait « Replier » : corrigé, test ajouté.
- **Non vérifié sur le PC** : R17 à R20 du plan : transparence réelle sous Windows (sans compositeur,
  Xvfb montre les coins en noir), mise à l'échelle 125/150 %, HUD au-dessus d'un jeu, F8 réel avec
  la souris sur le HUD.
- **Problèmes et dette** :
  1. Contrat : `Message` n'a qu'un texte et un niveau, donc ni titre précis (« Raccourci refusé »)
     ni bouton « Choisir une autre touche » comme sur la maquette ; à ajouter au contrat si voulu.
  2. Contrat : `Active` couvre « appui dans 3 s » et « bouton tenu » ; un état distinct permettrait
     d'écrire « enfoncé dans 3 s » puis « enfoncé » sur la ligne.
  3. Un clic sur le HUD lui donne le focus (le jeu le perd) ; le style Windows « no activate »
     l'éviterait (côté « programmation »), à juger au test sur le PC.
- **Prochaine étape** : test sur le PC (P1.6, R1 à R20) avec l'artefact de la CI, puis fusion par
  la session maître.

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
