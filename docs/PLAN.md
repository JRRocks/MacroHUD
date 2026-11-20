# Plan

> Statuts : ⬜ à faire · 🔄 en cours · ✅ terminé (critère vérifié, vérification écrite au journal) ·
> 🖥️ fait, à vérifier sur le PC · ⛔ bloqué (avec la raison).
> Aucune idée « pour plus tard » ici : elles vont dans `docs/idees-futur.md`.

## But

MacroHUD automatise le clavier et la souris sur le PC Windows, piloté par un petit HUD. Première
macro : maintenir un bouton de la souris enfoncé (D-04).

## Tâches

| N° | Tâche | Qui | Critère que l'utilisateur peut observer | Statut |
|---|---|---|---|---|
| P1.1 | Chaîne de livraison : la CI teste et compile à chaque push ; depuis `main`, publie `MacroHUD.<version>.exe` sur les Releases ; rapport de plantage lisible | programmation | Après un push sur `main`, une Release apparaît avec l'exécutable, à un numéro jamais vu | 🖥️ |
| P1.2 | Moteur « Maintenir le clic » : enfoncer, garder, relâcher ; bouton au choix ; clic physique = arrêt ; relâche à la fermeture et au plantage | programmation | Dans Paint, F8 trace comme si le bouton était tenu ; F8 ou un clic arrête | 🖥️ |
| P1.3 | Raccourcis : un par macro, F8 par défaut, modifiable, refus d'un raccourci déjà pris, réglages gardés | programmation | Changer le raccourci, relancer : le nouveau marche, l'ancien non | 🖥️ |
| P1.4 | Maquettes du HUD : cinq variantes | visuel | L'utilisateur en choisit une | ✅ |
| P1.5 | HUD validé codé (au-dessus des fenêtres, déplaçable, état, raccourci et options de chaque macro, version) | visuel | Le HUD ressemble à la maquette choisie | 🖥️ |
| P1.6 | Test sur le PC du travail des sessions cloud, point par point | test | Rapport de la session test sans échec | ⬜ |
| P1.7 | Première livraison : exécutable sur les Releases, miroir Drive | maître | L'utilisateur lance MacroHUD et garde un clic enfoncé avec F8 | ⬜ |

## À vérifier sur le PC

| N° | Depuis la version | Ce qu'il fait | Ce qu'il doit voir | État |
|---|---|---|---|---|
| R1 | — | Lance `MacroHUD.<version>.exe` | Le HUD s'affiche au-dessus des autres fenêtres | à essayer |
| R2 | — | Ouvre Paint, place la souris sur la toile, appuie sur F8 et bouge la souris | Un trait se dessine sans tenir le bouton ; le HUD montre la macro active | à essayer |
| R3 | — | Appuie à nouveau sur F8, ou clique | Le trait s'arrête ; le HUD montre la macro arrêtée | à essayer |
| R4 | — | Dans un jeu en fenêtre sans bordure | Le clic maintenu agit dans le jeu ; le HUD reste visible | à essayer |
| R5 | — | Dans le HUD : « Changer », puis Ctrl+Alt+M ; quitter et relancer | Message « Nouveau raccourci… : Ctrl+Alt+M » ; Ctrl+Alt+M bascule la macro, F8 ne fait plus rien, et ça tient après la relance | à essayer |
| R6 | — | « Changer », puis Win+E (pris par Windows) ; « Changer », puis Échap | Refus expliqué en français, l'ancien raccourci marche encore ; Échap : « Raccourci inchangé » | à essayer |
| R7 | — | Clavier AZERTY : « Changer », puis AltGr+E | Refus : Ctrl+Alt+E sert à écrire « € » | à essayer |
| R8 | — | « Activer » dans le HUD, puis mettre la souris sur la toile de Paint avant 3 s | Message « … dans 3 secondes » ; après 3 s le trait se dessine là où est la souris | à essayer |
| R9 | — | Pendant un maintien du bouton gauche, choisir « Droit » dans le HUD | Le maintien s'arrête, le bouton gauche est relâché, rien d'autre n'est enfoncé | à essayer |
| R10 | — | Pendant un maintien : « Quitter », ou Alt+F4 sur le HUD | Le bouton est relâché, MacroHUD se ferme | à essayer |
| R11 | — | Taper dans le Bloc-notes, puis F8 deux fois (souris hors du HUD) | Le Bloc-notes garde le clavier ; le HUD montre « active » puis « arrêtée » | à essayer |
| R12 | — | Déplacer le HUD, le replier, quitter, relancer | Le HUD revient à la même place, replié | à essayer |
| R13 | — | Lancer MacroHUD une seconde fois | Message « MacroHUD est déjà ouvert », la seconde copie se ferme | à essayer |
| R14 | — | Dans un terminal : `MacroHUD.<version>.exe --version` ; propriétés du fichier, onglet Détails | « MacroHUD 0.1.N », même numéro dans les propriétés | à essayer |
| R15 | — | Lancer avec `--essai-plantage`, F8 dans Paint, puis F8 | Le trait s'arrête ; une boîte « MacroHUD : plantage » donne le chemin du rapport dans `%APPDATA%\MacroHUD\plantages\` | à essayer |
| R16 | — | Pendant un maintien, Gestionnaire des tâches : fin de tâche du processus `macrohud`, puis relancer | Limite connue : le bouton reste enfoncé jusqu'à un clic sur ce bouton ; à la relance, le HUD le signale | à essayer |
| R17 | — | Lancer MacroHUD sur un bureau clair, puis au-dessus d'un jeu | Le HUD ressemble à `maquettes/rendu-egui/variante-2-egui.png` : coins arrondis, rien de gris ni de noir autour (fenêtre transparente) | à essayer |
| R18 | — | « Activer » dans le HUD | La ligne passe au vert (« Active · bouton gauche ») et un encadré bleu annonce l'appui dans 3 secondes | à essayer |
| R19 | — | Replier le HUD, puis appuyer sur F8 la souris posée sur la pastille | La pastille seule ; dessous, un encadré « Attention : Sortez la souris du HUD… » qui disparaît au bout de 6 s | à essayer |
| R20 | — | Affichage Windows à 125 % puis 150 % (Paramètres > Écran > Échelle) | Le HUD grandit d'autant, texte net, rien de coupé | à essayer |
