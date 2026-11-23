# CLAUDE.md : MacroHUD

Logiciel Windows qui automatise des actions du clavier et de la souris (comme AutoHotkey), piloté
depuis un petit HUD toujours visible, pour l'usage personnel de l'utilisateur sur son PC.

Les règles communes à tous mes projets viennent de la Library (`JRRocks/Library`, `SOCLE.md`).
Ce fichier ne contient que ce qui est propre à ce projet. Il reste court : le détail va dans `docs/`.

## Mémoire du projet

| Fichier | Contenu |
|---|---|
| `docs/PLAN.md` | Tâches numérotées, statut, critères, liste « à vérifier sur le PC » |
| `docs/JOURNAL.md` | Une entrée par session, la plus récente en haut |
| `docs/DECISIONS.md` | Décisions numérotées et datées |
| `docs/CONTRAT-HUD.md` | Ce que le HUD montre et ce qu'il peut demander : la frontière entre code et visuel |
| `docs/MACROS-EXTERNES.md` | Macros externes : scripts du PC, hors du dépôt, pilotés par le HUD (déclaration, échanges, arrêt) |
| `docs/idees-futur.md` | Idées pour plus tard : à ne lire que sur ma demande |

## Qui fait quoi (D-07)

| Session | Rôle | Ce qu'elle touche |
|---|---|---|
| Maître (PC) | Cadre, valide avec l'utilisateur, fusionne dans `main`, livre, tient la mémoire, miroir Drive. Ne code pas | `CLAUDE.md`, `docs/` |
| Cloud « programmation » | Tout le code non visuel : moteur des macros, raccourcis, entrées souris et clavier, réglages, CI | tout sauf `src/hud/` et `maquettes/` |
| Cloud « visuel » | Maquettes du HUD (cinq variantes), puis le HUD validé | `maquettes/`, `src/hud/` |
| Locale « test » (PC) | Vérifie sur Windows le travail des sessions cloud et rend un rapport. Ne modifie pas le code | `essais-prives/` (ignoré par git) |

Sessions cloud : chacune sur sa branche `claude/…`, jamais de pull request, jamais de push sur `main`.

## Commandes

```bash
cargo test                # vérifier la logique (tourne aussi sous Linux)
cargo build --release     # compiler l'exécutable (sur Windows ou dans la CI)
cargo fmt --check                                                          # mise en forme
cargo clippy --all-targets -- -D warnings                                  # avertissements = erreurs
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings  # code Windows, depuis Linux
cargo run                 # le HUD sous Linux, souris et clavier simulés (pour la session « visuel »)
```

- Options de l'exécutable : `--version`, `--aide`, `--essai-plantage` (essais sur le PC : plante
  exprès quand on arrête un maintien, pour vérifier que le bouton est relâché et le rapport écrit).
- Le HUD : `src/hud/` (session « visuel » ; son interface avec le reste est décrite en tête de
  `src/hud/mod.rs`). Ce qu'il reçoit et demande : `src/contract.rs`, traduction de `docs/CONTRAT-HUD.md`.
- Sur le PC, `%APPDATA%\MacroHUD\` : `reglages.json`, `macrohud.log` (journal d'exécution ; celui
  du lancement précédent : `macrohud.precedent.log`), `plantages\` (rapports de plantage datés).

## Livraison

- Canal : GitHub Releases du dépôt, fichier `MacroHUD.<version>.exe` (Windows x64, un seul
  exécutable), produit par la CI à chaque push sur `main`, tests passés d'abord.
- Version : monte toute seule à chaque compilation de la CI ; jamais deux livraisons au même numéro.
  Affichée dans le HUD et par `macrohud --version`.
- Version = `0.1.<numéro d'exécution de la CI>` (aussi dans les propriétés du fichier) ; une
  compilation locale dit `0.1.0-dev`. Ne pas renommer `.github/workflows/ci.yml` : les numéros
  repartiraient de 1. Relancer une exécution (« Re-run ») garde son numéro : sur `main`, si la
  Release de ce numéro existe déjà, la publication échoue au lieu de livrer deux fois le même numéro.
- À chaque push sur n'importe quelle branche, la CI teste puis compile ; `MacroHUD.<version>.exe` est
  alors un artefact de l'exécution (onglet Actions) : c'est lui que la session « test » essaie avant
  une fusion. Sur `main` seulement, et seulement si tout est passé : Release `v<version>`.
- Mise à jour : remplacer l'exécutable ; les réglages restent dans `%APPDATA%\MacroHUD`.
- Signature : aucune (Windows SmartScreen peut avertir au premier lancement).
- Dépôt : **public** ; `main` ne reçoit que les fusions de la session maître, après le test local
  et l'accord de l'utilisateur.

## Sauvegarde

- Miroir Drive : dossier `MacroHUD/` du dossier Drive « Claude Code » (chemin exact dans le
  `CLAUDE.md` de ce dossier Drive). Du PC vers le Drive seulement, jamais depuis une session cloud :
  `robocopy "<dossier local>" "<lecteur>:\…\Claude Code\MacroHUD" /MIR /XD .git target essais-prives`

## Conventions

- Textes affichés (HUD, messages, erreurs) : français avec accents ; code et commentaires : anglais ;
  messages de commit : français.
- Identité des commits : `JRRocks <314636139+JRRocks@users.noreply.github.com>`.

## Ce qui compte vraiment dans ce code

- **Un bouton ne reste jamais coincé.** Arrêt de la macro, fermeture, plantage : MacroHUD relâche
  toujours ce qu'il a enfoncé. Un clic gauche bloqué rend le PC inutilisable.
- **Un clic physique reprend la main.** Si l'utilisateur appuie lui-même sur le bouton maintenu, la
  macro s'arrête et le HUD l'affiche.
- **Chaque macro a son raccourci, choisi dans le HUD** (F8 par défaut pour la première) : le HUD est
  pensé pour plusieurs macros, même s'il n'y en a qu'une (D-05).
- **Rien de visible sans aperçu validé** : le HUD se montre d'abord en maquettes (cinq variantes) et
  ne se code qu'après le « oui » de l'utilisateur.
- **Dépôt public : rien de privé.** Aucune capture du bureau de l'utilisateur, aucun rapport de test
  du PC dans le dépôt : ils vivent dans `essais-prives/`, ignoré par git.
- **Pendant un test sur le PC, les autres sessions Claude font une halte** (D-14). La session
  maître leur demande une pause avant le test souris et clavier, attend leur repos, et leur rend la
  main après : une action d'une autre session fausserait le résultat.

## Ce que seul le PC peut vérifier

Le lancement de l'exécutable sur Windows réel, le HUD au-dessus des autres fenêtres (et d'un jeu en
plein écran fenêtré), le raccourci global, le clic maintenu reçu par une vraie application ou un jeu,
l'accueil de SmartScreen et de l'antivirus.
