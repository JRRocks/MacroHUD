# Macros externes

Une macro externe est un script du PC que MacroHUD lance, arrête et montre dans son HUD comme ses
propres macros (D-32). Le script et sa déclaration restent hors du dépôt (D-34) : le dépôt ne
contient que ce mécanisme et un script d'exemple pour les tests. Ce fichier est la règle commune
des sessions « programmation » (moteur) et « visuel » (HUD) ; le HUD suit en plus
`docs/CONTRAT-HUD.md` et l'aperçu validé de la ligne (P4.3).

## Déclaration

Un fichier par macro : `%APPDATA%\MacroHUD\macros-externes\<id>.json` (UTF-8). `<id>` (nom du
fichier sans `.json`) identifie la macro dans `reglages.json`. Lu au démarrage de MacroHUD ; une
ligne par fichier valide, sous les macros internes, dans l'ordre alphabétique des `<id>`.

```json
{
  "nom": "Exemple",
  "description": "Clique toutes les deux secondes",
  "commande": "powershell.exe",
  "arguments": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "C:\\…\\exemple.ps1"],
  "dossier": "C:\\…",
  "raccourci": "F10",
  "boutons": ["droit"],
  "reglages": [
    { "cle": "limite", "nom": "Limite de clics", "unite": "clics", "min": 10, "max": 500, "pas": 10, "defaut": 100 }
  ],
  "etat_arretee": "limite {limite} clics",
  "info": "Garde la fenêtre visée au premier plan. Un vrai clic n'arrête pas cette macro : F10 ou ▶ l'arrêtent."
}
```

- Obligatoires : `nom`, `commande`. `arguments`, `dossier` (dossier de travail), `description`,
  `info` : facultatifs. `raccourci` : touche proposée par défaut ; l'utilisateur la change dans le HUD
  comme pour les autres macros (refus d'une touche déjà prise, réglage gardé).
- `boutons` : les boutons de la souris que le script peut enfoncer (`gauche`, `droit`, `milieu`).
  MacroHUD les relâche s'il doit arrêter le script de force (voir « Arrêt »).
- `reglages` : des nombres entiers seulement, montrés en curseurs comme la rapidité de
  « Multi-clic » ; valeur gardée dans `reglages.json`, `defaut` au premier lancement.
- `etat_arretee` : texte montré après « ■ Arrêtée · » quand la macro est arrêtée ; `{cle}` y
  devient la valeur du réglage. Sans lui, ou quand la macro ne peut pas démarrer : « ■ Arrêtée ».
- Un fichier illisible ou incomplet ne bloque rien : MacroHUD l'écrit dans `macrohud.log` et montre
  les autres macros.

## Échanges avec le script

Le script tourne seulement pendant que la macro est active. MacroHUD lui parle par son entrée
standard, il répond par sa sortie standard : une ligne de texte UTF-8 par message, mot-clé en tête.

MacroHUD vers le script :

| Ligne | Quand | Le script doit |
|---|---|---|
| `reglage <cle> <valeur>` | au lancement, une par réglage ; puis à chaque changement dans le HUD | appliquer la valeur tout de suite (comme la rapidité, D-27) |
| `pause` | la souris entre dans le HUD | ne plus cliquer (D-22) |
| `reprise` | la souris quitte le HUD | reprendre |
| `arret` | arrêt demandé (voir « Arrêt ») | finir son geste, relâcher ce qu'il tient, quitter (code 0) |
| entrée fermée | MacroHUD fermé ou planté | même chose qu'`arret` |

Le script vers MacroHUD :

| Ligne | Ce que montre le HUD |
|---|---|
| `etat <texte>` | « ● Active · <texte> » (ex. « 12/100 ») |
| `pastille <couleur> <texte>` | pastille d'étape à côté de la touche ; couleur `ambre`, `verte`, `bleue` ou `grise` ; `pastille` seul l'efface |
| `attente <texte>` | « ❚❚ En pause · <texte> » (le script attend une condition, ex. sa fenêtre au premier plan) |
| `reprise` | retour à « ● Active · … » |
| `alerte <titre> \| <texte>` | encadré ambre sous la ligne, comme « Raccourci refusé » ; reste jusqu'au lancement suivant |
| `fin <texte>` | le script s'arrête de lui-même : « ✓ Terminée · <texte> », liseré bleu, jusqu'au lancement suivant |
| `journal <texte>` | une ligne dans `macrohud.log`, préfixée du nom de la macro |

Toute autre ligne va dans `macrohud.log`. Une ligne trop longue est coupée à 200 caractères.

## Arrêt

- Arrêt demandé par le raccourci, le bouton du HUD, l'activation d'une autre macro (une macro à la
  fois, D-20), « Quitter » ou Alt+F4 : MacroHUD écrit `arret`, attend la fin du script 2 s au plus,
  puis l'arrête de force et relâche ceux des `boutons` déclarés encore enfoncés. Le journal dit
  lequel des deux a eu lieu.
- Script qui s'arrête seul sans `fin` (erreur, plantage) : boutons déclarés relâchés de même ; la
  ligne repasse à « arrêtée » avec l'encadré « « <nom> » s'est arrêtée seule » et le code de sortie.
- Ajout du 2026-11-23 : un script qui a envoyé une `alerte` pendant ce lancement, puis s'arrête seul
  sans `fin`, garde son encadré, seul, quel que soit son code de sortie (écrit dans `macrohud.log`) ;
  pas d'encadré « s'est arrêtée seule » en plus (aperçu validé, vue 7). Celui-ci ne sert qu'à une
  sortie sans `fin` ni `alerte`. Les boutons déclarés sont relâchés dans tous les cas.
- Script introuvable ou qui ne démarre pas : la ligne reste « arrêtée » avec l'encadré « La macro ne
  peut pas démarrer » et la raison ; les autres macros marchent.
- Un vrai clic de l'utilisateur n'arrête pas une macro externe et passe normalement (D-35).
- Aucun clic injecté n'atteint le HUD : en plus de `pause`, le crochet de la souris écarte les clics
  injectés sous lesquels se trouve le HUD (D-22).

## HUD

Comme une macro interne : bouton ▶/■, nom, état écrit, touche, flèche des réglages (les curseurs
de `reglages`, le raccourci, la ligne `info`). En plus : la pastille d'étape, l'état « En pause »
venu du script, l'état « Terminée », l'encadré ambre. Replié pendant la macro : la capsule montre
l'état et la pastille. Le détail visuel suit l'aperçu validé le 2026-11-23 (D-37) :
`maquettes/macro-externe/apercu.html`.

Depuis le 2026-11-25 (D-38, D-39) : une macro active qui montre une pastille la porte sur la ligne
du nom, contre la touche, et garde son texte d'état entier :
`maquettes/macro-externe/pastille-ligne-du-nom.html`.

## Tests

Un script d'exemple dans le dépôt (`tests/macro-exemple/`), sans rapport avec un logiciel précis,
qui parle ce protocole : il couvre le lancement, les réglages, la pastille, `attente`, `fin`,
l'arrêt demandé, l'arrêt forcé après 2 s, la sortie en erreur et la fermeture de l'entrée.
