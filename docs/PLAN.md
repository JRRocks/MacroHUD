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
| P1.1 | Chaîne de livraison : la CI teste et compile à chaque push ; depuis `main`, publie `MacroHUD.<version>.exe` sur les Releases ; rapport de plantage lisible | programmation | Après un push sur `main`, une Release apparaît avec l'exécutable, à un numéro jamais vu | ⬜ |
| P1.2 | Moteur « Maintenir le clic » : enfoncer, garder, relâcher ; bouton au choix ; clic physique = arrêt ; relâche à la fermeture et au plantage | programmation | Dans Paint, F8 trace comme si le bouton était tenu ; F8 ou un clic arrête | ⬜ |
| P1.3 | Raccourcis : un par macro, F8 par défaut, modifiable, refus d'un raccourci déjà pris, réglages gardés | programmation | Changer le raccourci, relancer : le nouveau marche, l'ancien non | ⬜ |
| P1.4 | Maquettes du HUD : cinq variantes | visuel | L'utilisateur en choisit une | 🔄 |
| P1.5 | HUD validé codé (au-dessus des fenêtres, déplaçable, état, raccourci et options de chaque macro, version) | visuel | Le HUD ressemble à la maquette choisie | 🔄 |
| P1.6 | Test sur le PC du travail des sessions cloud, point par point | test | Rapport de la session test sans échec | ⬜ |
| P1.7 | Première livraison : exécutable sur les Releases, miroir Drive | maître | L'utilisateur lance MacroHUD et garde un clic enfoncé avec F8 | ⬜ |

## À vérifier sur le PC

| N° | Depuis la version | Ce qu'il fait | Ce qu'il doit voir | État |
|---|---|---|---|---|
| R1 | — | Lance `MacroHUD.<version>.exe` | Le HUD s'affiche au-dessus des autres fenêtres | à essayer |
| R2 | — | Ouvre Paint, place la souris sur la toile, appuie sur F8 et bouge la souris | Un trait se dessine sans tenir le bouton ; le HUD montre la macro active | à essayer |
| R3 | — | Appuie à nouveau sur F8, ou clique | Le trait s'arrête ; le HUD montre la macro arrêtée | à essayer |
| R4 | — | Dans un jeu en fenêtre sans bordure | Le clic maintenu agit dans le jeu ; le HUD reste visible | à essayer |
