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
