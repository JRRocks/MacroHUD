# Plan

> Statuts : ⬜ à faire · 🔄 en cours · ✅ terminé (critère vérifié, vérification écrite au journal) ·
> 🖥️ fait, à vérifier sur le PC · ⛔ bloqué (avec la raison).
> Aucune idée « pour plus tard » ici : elles vont dans `docs/idees-futur.md`.

## But

MacroHUD automatise le clavier et la souris sur le PC Windows, piloté par un petit HUD. Première
macro : maintenir un bouton de la souris enfoncé (D-04). Deuxième macro : « Multi-clic », des clics
répétés à la rapidité choisie (D-18).

## Tâches

| N° | Tâche | Qui | Critère que l'utilisateur peut observer | Statut |
|---|---|---|---|---|
| P1.1 | Chaîne de livraison : la CI teste et compile à chaque push ; depuis `main`, publie `MacroHUD.<version>.exe` sur les Releases ; rapport de plantage lisible | programmation | Après un push sur `main`, une Release apparaît avec l'exécutable, à un numéro jamais vu | ✅ |
| P1.2 | Moteur « Maintenir le clic » : enfoncer, garder, relâcher ; bouton au choix ; clic physique = arrêt ; relâche à la fermeture et au plantage | programmation | Dans Paint, F8 trace comme si le bouton était tenu ; F8 ou un clic arrête | 🖥️ reste le vrai clic (R3) |
| P1.3 | Raccourcis : un par macro, F8 par défaut, modifiable, refus d'un raccourci déjà pris, réglages gardés | programmation | Changer le raccourci, relancer : le nouveau marche, l'ancien non | ✅ |
| P1.4 | Maquettes du HUD : cinq variantes | visuel | L'utilisateur en choisit une | ✅ |
| P1.5 | HUD validé codé (au-dessus des fenêtres, déplaçable, état, raccourci et options de chaque macro, version) | visuel | Le HUD ressemble à la maquette choisie | 🖥️ écarts corrigés (P1.9) |
| P1.6 | Test sur le PC du travail des sessions cloud, point par point | test | Rapport de la session test sans échec | ✅ passage 2 |
| P1.7 | Première livraison : exécutable sur les Releases, miroir Drive | maître | L'utilisateur lance MacroHUD et garde un clic enfoncé avec F8 | 🖥️ v0.1.10 publiée |
| P1.8 | Relâchement reçu par l'application qui a reçu l'appui, quel que soit l'arrêt : clic sur le HUD, « Quitter », Alt+F4, changement de bouton, plantage (D-15) | programmation | Maintien droit commencé sur une fenêtre, clic sur le HUD : la fenêtre reçoit le relâchement, comme avec F8 | 🖥️ R24 |
| P1.9 | HUD conforme à la variante 2 (D-16) : « Active · gauche enfoncé » ; encadré ambre « Raccourci refusé », triangle, texte court, bouton « Choisir une autre touche » ; aucun liseré autour de la fenêtre | programmation (contrat, textes) puis visuel | Le vrai HUD et `maquettes/variante-2.png` ne diffèrent plus que par la version et le nombre de macros | 🖥️ contrat, textes, encadré ambre et liseré faits : R25, R26 et comparaison à la maquette (passage 3) |
| P2.1 | Moteur « Multi-clic » (D-18 à D-22) : clics répétés à la rapidité réglée, là où est la souris ; arrêt par la touche, par un vrai clic du même bouton ou par le HUD ; une macro à la fois ; aucun clic sur le HUD ; jamais de bouton coincé (arrêt, fermeture, plantage) ; journal des démarrages et des arrêts | programmation | Sur la fenêtre d'essai, 10 clics/s pendant 10 s donnent 100 clics, à 5 % près ; F8 pendant le multi-clic l'arrête et lance le maintien | ⬜ |
| P2.2 | Aperçu du HUD avec « Multi-clic » (page sans script) : le HUD actuel, puis le HUD à deux macros en variantes A, B, C du réglage de la rapidité, rendues avec egui | visuel | L'utilisateur choisit une variante | ⬜ |
| P2.3 | HUD de « Multi-clic » codé selon la variante choisie ; rendu réel placé à côté de l'aperçu | visuel | Le rendu du HUD et la variante choisie ne diffèrent pas | ⬜ |
| P2.4 | Test sur le PC, passage 3 : corrections (R24 à R26), « Multi-clic » (R27 à R34), reprise des points du passage 2 | test | Rapport sans échec | ⬜ |
| P2.5 | Livraison (D-21) : nouvelle version sur les Releases avec les corrections et « Multi-clic », miroir Drive, exécutable posé dans le dossier d'applications de l'utilisateur | maître | L'utilisateur lance la nouvelle version et règle la rapidité | ⬜ |

## À vérifier sur le PC

> « OK » : vérifié par la session « test » au passage 2, sur l'exécutable 0.1.9 de la CI (même code
> que 0.1.10), avec une fenêtre d'essai qui compte les messages de la souris à la place de Paint et
> du Bloc-notes. « toi » : seul l'utilisateur peut le vérifier.

| N° | Depuis la version | Ce qu'il fait | Ce qu'il doit voir | État |
|---|---|---|---|---|
| R1 | 0.1.10 | Lance `MacroHUD.<version>.exe` | Le HUD s'affiche au-dessus des autres fenêtres | OK |
| R2 | 0.1.10 | Ouvre Paint, place la souris sur la toile, appuie sur F8 et bouge la souris | Un trait se dessine sans tenir le bouton ; le HUD montre la macro active | OK |
| R3 | 0.1.10 | Appuie à nouveau sur F8, ou clique | Le trait s'arrête ; le HUD montre la macro arrêtée | F8 OK ; vrai clic : toi |
| R4 | 0.1.10 | Dans un jeu en fenêtre sans bordure | Le clic maintenu agit dans le jeu ; le HUD reste visible | toi |
| R5 | 0.1.10 | Dans le HUD : « Changer », puis Ctrl+Alt+M ; quitter et relancer | Message « Nouveau raccourci… : Ctrl+Alt+M » ; Ctrl+Alt+M bascule la macro, F8 ne fait plus rien, et ça tient après la relance | OK (avec F9) |
| R6 | 0.1.10 | « Changer », puis Win+E (pris par Windows) ; « Changer », puis Échap | Refus expliqué en français, l'ancien raccourci marche encore ; Échap : « Raccourci inchangé » | OK (avec Alt+Tab, Ctrl+C) ; Win+E pas essayé |
| R7 | 0.1.10 | Clavier AZERTY : « Changer », puis AltGr+E | Refus : Ctrl+Alt+E sert à écrire « € » | pas essayé |
| R8 | 0.1.10 | « Activer » dans le HUD, puis mettre la souris sur la toile de Paint avant 3 s | Message « Activation dans 3 s… » qui décompte (2 s, 1 s) ; la macro reste « arrêtée » jusqu'à l'appui ; après 3 s le trait se dessine là où est la souris | OK |
| R9 | 0.1.10 | Pendant un maintien du bouton gauche, choisir « Droit » dans le HUD | Le maintien s'arrête, le bouton gauche est relâché, rien d'autre n'est enfoncé (ce clic arrête ; « Droit » se choisit au clic suivant) | OK |
| R10 | 0.1.10 | Pendant un maintien : « Quitter », ou Alt+F4 sur le HUD | « Quitter » : le premier clic arrête le maintien (bouton relâché), le second ferme MacroHUD ; Alt+F4 : bouton relâché et fermeture | OK ; l'application ne reçoit pas le relâchement (P1.8) |
| R11 | 0.1.10 | Taper dans le Bloc-notes, puis F8 deux fois (souris hors du HUD) | Le Bloc-notes garde le clavier ; le HUD montre « active » puis « arrêtée » | OK |
| R12 | 0.1.10 | Déplacer le HUD, le replier, quitter, relancer | Le HUD revient à la même place, replié | OK |
| R13 | 0.1.10 | Lancer MacroHUD une seconde fois | Message « MacroHUD est déjà ouvert », la seconde copie se ferme | OK |
| R14 | 0.1.10 | Dans un terminal : `MacroHUD.<version>.exe --version` ; propriétés du fichier, onglet Détails | « MacroHUD 0.1.N », même numéro dans les propriétés | OK |
| R15 | 0.1.10 | Lancer avec `--essai-plantage`, F8 dans Paint, puis F8 | Le trait s'arrête ; une boîte « MacroHUD : plantage » donne le chemin du rapport dans `%APPDATA%\MacroHUD\plantages\` | OK |
| R16 | 0.1.10 | Pendant un maintien, Gestionnaire des tâches : fin de tâche du processus `macrohud`, puis relancer | Limite connue : le bouton reste enfoncé jusqu'à un clic sur ce bouton ; à la relance, le HUD le signale | OK (limite confirmée) |
| R17 | 0.1.10 | Lancer MacroHUD sur un bureau clair, puis au-dessus d'un jeu | Le HUD ressemble à `maquettes/rendu-egui/variante-2-egui.png` : coins arrondis, rien de gris ni de noir autour (fenêtre transparente) | fond clair OK sauf liseré (P1.9) ; jeu : toi |
| R18 | 0.1.10 | « Activer » dans le HUD | La ligne reste « arrêtée » ; un encadré bleu décompte « Activation dans 3 s… », 2 s, 1 s ; à l'appui la ligne passe au vert et l'encadré disparaît | OK |
| R19 | 0.1.10 | Replier le HUD, puis appuyer sur F8 la souris posée sur la pastille | La pastille seule ; dessous, un encadré « Attention : Sors la souris du HUD… » qui disparaît au bout de 6 s | OK (avec F9) |
| R20 | 0.1.10 | Affichage Windows à 125 % puis 150 % (Paramètres > Écran > Échelle) | Le HUD grandit d'autant, texte net, rien de coupé | toi (un test ne change pas les réglages de Windows) |
| R21 | 0.1.10 | « Changer », puis F9 sans cliquer ailleurs (le HUD garde le focus) ; de même avec Ctrl+C, Échap, puis F8 | F9 : « Nouveau raccourci… : F9 » ; Ctrl+C : refus expliqué ; Échap et F8 : « Raccourci inchangé » (F8 n'active pas la macro) ; `macrohud.log` dit par quelle voie la touche est arrivée | OK |
| R22 | 0.1.10 | Pendant un maintien du bouton droit (puis du milieu) commencé sur Paint, cliquer « Arrêter » ou « Quitter » dans le HUD | Le premier clic arrête le maintien (bouton relâché, message « … arrêtée par ton clic sur le HUD… ») et n'arrive pas à Paint ; le clic suivant agit sur le HUD | OK ; l'application ne reçoit pas le relâchement (P1.8) |
| R23 | 0.1.10 | « Activer », puis F8 pendant le décompte | Le décompte s'arrête, son encadré disparaît, rien n'est enfoncé | OK |
| R24 | — | Maintien commencé sur la fenêtre d'essai, du bouton gauche, puis droit, puis du milieu ; l'arrêter de chaque façon : F8 la souris sur la fenêtre, F8 la souris sur le HUD, clic sur le HUD (« Arrêter », « Quitter », choix d'un autre bouton), Alt+F4 sur le HUD, F8 la souris sur le HUD après un lancement avec `--essai-plantage` ; puis de même dans Paint | À chaque arrêt, la fenêtre reçoit exactement un relâchement ; `macrohud.log` a, pour chaque appui, une ligne « Appui du … » et, pour chaque arrêt, une ligne « Relâchement du … » qui dit la fenêtre sous le curseur, si l'application de l'appui tenait la capture de la souris, si elle était au premier plan, et si MacroHUD lui a envoyé le relâchement | à essayer |
| R25 | — | « Changer », puis Ctrl+C ; « Changer », puis Alt+Tab ; « Changer », puis AltGr+E (clavier AZERTY) | Avertissement court qui dit la touche refusée et le raccourci qui reste : « Ctrl+C sert à copier. F8 reste actif. », « Alt+Tab est déjà pris par Windows. F8 reste actif. », « Ctrl+Alt+E sert à écrire « € ». F8 reste actif. » (l'encadré ambre « Raccourci refusé » et son bouton viennent ensuite de la session « visuel ») | à essayer |
| R26 | — | Lancer MacroHUD sur un bureau clair (fond blanc), regarder de près les bords et les coins du HUD, déplié puis replié ; puis au-dessus d'un jeu | Aucune ombre, aucun liseré, rien d'assombri autour du HUD ; hors de la forme arrondie, les coins laissent voir le bureau (ni gris ni noir) ; `macrohud.log` : « Fenêtre du HUD : ombre de Windows : retirée ; bordure de Windows 11 : retirée ; coins arrondis par Windows : non ; zone dessinée par le HUD : toute la fenêtre. » | à essayer |
| R27 | — | La souris posée sur la fenêtre d'essai (puis sur la toile de Paint), F9 ; puis F9 | Des clics répétés là où est la souris, à la rapidité affichée (à 10 clics/s : 100 clics en 10 s, à 5 % près) ; le HUD montre « Multi-clic » active, puis arrêtée ; rien ne reste enfoncé | à essayer |
| R28 | — | Pendant le multi-clic, régler la rapidité à 1, puis à 15 clics/s ; quitter, relancer | La cadence suit dès le clic suivant ; la rapidité choisie revient après la relance | à essayer |
| R29 | — | Pendant le multi-clic du bouton gauche : un vrai clic gauche ; réactiver, puis un vrai clic droit | Le clic gauche arrête la macro et n'arrive pas à l'application ; le clic droit passe normalement et les clics continuent | toi (vrai clic) |
| R30 | — | Pendant le multi-clic : F8 ; puis, pendant le maintien : F9 | F8 arrête le multi-clic et lance le maintien ; F9 relâche le bouton maintenu et lance le multi-clic ; jamais les deux à la fois | à essayer |
| R31 | — | Pendant le multi-clic, amener la souris sur le HUD, puis la ressortir ; macro arrêtée, F9 la souris sur le HUD | Aucun clic n'atteint le HUD et les clics reprennent dehors ; F9 sur le HUD n'active rien et un message demande de sortir la souris du HUD | à essayer |
| R32 | — | « Activer » sur la ligne « Multi-clic », puis la souris sur la fenêtre d'essai avant 3 s | Décompte « Activation dans 3 s… », puis les clics commencent là où est la souris | à essayer |
| R33 | — | Pendant le multi-clic : « Arrêter » ou « Quitter » dans le HUD, Alt+F4 sur le HUD ; puis lancement avec `--essai-plantage` et arrêt du multi-clic | À chaque arrêt, plus aucun clic et aucun bouton enfoncé ; le premier clic sur le HUD arrête, le second agit ; Alt+F4 et le second clic sur « Quitter » ferment MacroHUD ; plantage : boîte « MacroHUD : plantage » et rapport écrit | à essayer |
| R34 | — | Dans un jeu en fenêtre sans bordure | Les clics répétés agissent dans le jeu, à la rapidité choisie ; le HUD reste visible | toi |
