# Macro externe d'exemple

La seule macro externe du dépôt : « Exemple », un script PowerShell générique, sans rapport avec un
logiciel précis, qui parle le protocole de `docs/MACROS-EXTERNES.md`. Il fait semblant de cliquer
toutes les deux secondes jusqu'à sa limite de clics ; il ne touche ni à la souris ni au clavier,
sauf si on le lui demande (`-Clicks`, pour les essais sur le PC).

| Fichier | Rôle |
|---|---|
| `exemple.ps1` | Le script (Windows PowerShell 5.1 et PowerShell 7). Enregistré en UTF-8 **avec BOM** : sans lui, Windows PowerShell 5.1 lit mal ses accents |
| `exemple.json` | La déclaration à copier sur le PC (chemins à adapter) |
| `essais.rs` | Ses essais : le vrai moteur de MacroHUD le lance, lui parle et l'arrête. Compilés dans `cargo test` par `src/main.rs` |

## Les essais

`cargo test` les lance avec chaque PowerShell présent (Windows : `powershell.exe` et `pwsh.exe` ;
ailleurs : `pwsh`). Sans PowerShell, ils sont sautés, sauf dans la CI, où ils échouent. Ils couvrent
le lancement, les réglages, la pastille, `attente`, `fin`, l'arrêt demandé, l'arrêt forcé après 2 s,
la sortie en erreur, la fermeture de l'entrée, une alerte suivie d'une sortie, la fermeture de
MacroHUD, le script ou la commande introuvables.

## La déclarer sur le PC

1. Copier `exemple.json` dans `%APPDATA%\MacroHUD\macros-externes\exemple.json`.
2. Y remplacer les deux chemins `C:\Users\<utilisateur>\…\MacroHUD\tests\macro-exemple` par le
   dossier de ce fichier sur le PC (les `\` s'écrivent `\\` en JSON).
3. Relancer MacroHUD : les déclarations sont lues au démarrage. La ligne « Exemple » vient sous les
   macros de MacroHUD ; F10 la lance et l'arrête. Une déclaration refusée est expliquée dans
   `%APPDATA%\MacroHUD\macrohud.log`.

Pour une variante, une autre déclaration dans le même dossier (le nom du fichier est l'identifiant
de la macro), par exemple `exemple-sourd.json` avec `"nom": "Exemple sourd"`, un autre `raccourci`,
et les options voulues ajoutées après le chemin du script dans `arguments`.

## Options du script

| Option | Effet |
|---|---|
| `-Mode normal` | Par défaut : compte ses clics jusqu'à la limite, puis `fin` |
| `-Mode wait` | Attend d'abord (« En pause · attend sa fenêtre »), puis comme `normal` |
| `-Mode error` | Quitte en erreur après son premier clic (code 3), sans `fin` |
| `-Mode alert` | Envoie une alerte, puis quitte (code 1) sans `fin` |
| `-Mode deaf` | N'entend ni `arret` ni la fermeture de son entrée : MacroHUD doit l'arrêter de force |
| `-Interval <ms>` | Temps entre deux clics (2000 par défaut) |
| `-ExitCode <n>` | Code de sortie des modes `error` et `alert` |
| `-Clicks click` | Un vrai clic de `-Button` à chaque fois (Windows) |
| `-Clicks hold` | Tient `-Button` enfoncé de son premier clic à sa fin (Windows) ; le relâche en pause |
| `-Button left\|right\|middle` | Le bouton des vrais clics (`right` par défaut, le bouton déclaré) |
| `-Stubborn` | N'obéit pas à `pause` : pour vérifier que MacroHUD écarte les clics injectés sur son HUD |

Avec de vrais clics, déclarer le bouton utilisé dans `boutons` : c'est lui que MacroHUD relâche
après un arrêt forcé, une sortie sans `fin` ou un plantage.
