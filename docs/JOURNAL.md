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
