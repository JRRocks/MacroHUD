# Décisions

> Une décision ne se réécrit pas. Pour la changer : une ligne nouvelle qui la remplace, et la
> mention « remplacée par D-xx » sur l'ancienne.
> Colonne « Qui » : `utilisateur` pour ce qu'il a tranché, `développeur` pour un choix technique
> sur lequel il peut revenir.

| N° | Date | Décision | Pourquoi | Qui |
|---|---|---|---|---|
| D-01 | 2026-11-20 | Logiciel Windows qui automatise clavier et souris, avec un petit HUD pour contrôler les macros et voir leur état. Pour l'usage personnel de l'utilisateur, hors ligne, sans compte. | Demande initiale (« comme AutoHotkey »). | utilisateur |
| D-02 | 2026-11-20 | Nom : MacroHUD (titre du HUD, exécutable, dépôt). | Choix au cadrage. | utilisateur |
| D-03 | 2026-11-20 | Langage : Rust partout ; HUD en egui, comme Massicot. | Un seul exécutable léger sans dépendance, même chaîne de compilation et de livraison que Massicot. | utilisateur |
| D-04 | 2026-11-20 | Première macro, « Maintenir le clic » : F8 enfonce le bouton de la souris et le garde enfoncé ; F8 à nouveau, ou un clic physique, le relâche. Bouton gauche par défaut, modifiable dans le HUD. | Seul besoin actuel. | utilisateur |
| D-05 | 2026-11-20 | Chaque macro a son propre raccourci, choisi dans le HUD ; F8 par défaut pour la première. Le HUD est conçu pour accueillir plusieurs macros. | « Développer des choix dans l'UI pour quand plusieurs macros seront en place. » | utilisateur |
| D-06 | 2026-11-20 | Dépôt GitHub public. | Compilation Windows gratuite et illimitée ; rien de personnel dans le dépôt. | utilisateur |
| D-07 | 2026-11-20 | Organisation : la session maître (PC) cadre, valide, fusionne, livre et ne code pas ; une session cloud « programmation » fait tout le code ; une session cloud « visuel » fait les maquettes puis le HUD ; une session locale « test » vérifie sur le PC le travail des sessions cloud, sans modifier le code. | Demande de l'utilisateur. | utilisateur |
| D-08 | 2026-11-20 | Textes affichés en français avec accents ; code et commentaires en anglais ; messages de commit en français. | Convention de Massicot. | développeur |
| D-09 | 2026-11-20 | Livraison : un seul exécutable `MacroHUD.<version>.exe` sur les Releases GitHub, produit par la CI à chaque push sur `main`, tests d'abord, version qui monte seule. Réglages dans `%APPDATA%\MacroHUD` : remplacer l'exécutable garde les réglages. | Méthode commune (livraison dès le premier jour, jamais deux fois le même numéro). | développeur |
| D-10 | 2026-11-20 | Sessions cloud sur leur branche `claude/…`, sans pull request ; fusion dans `main` par la session maître après le test local et l'accord de l'utilisateur. | Pas de pull request sans demande ; rien dans `main` qui n'ait été essayé sur le PC. | développeur |
| D-11 | 2026-11-20 | La frontière entre code et visuel est `docs/CONTRAT-HUD.md` : la session « programmation » l'expose en types Rust, la session « visuel » ne dessine que ce qu'il contient. | Deux sessions travaillent en parallèle sans se marcher dessus. | développeur |
| D-12 | 2026-11-20 | HUD : variante 2 « Liste compacte » des maquettes (`maquettes/variante-2.png`), telle quelle : une ligne par macro (bouton ▶, état, touche du raccourci à droite), réglages sous la ligne, forme repliée en pastille. | Choisie parmi cinq variantes ; la plus petite sur un jeu, grandit d'une ligne par macro. | utilisateur |
| D-13 | 2026-11-20 | Les tests sur le PC (moteur, puis HUD final) se lancent seuls dès que le code est prêt, sans redemander. | Accord de l'utilisateur ; il ne touche pas à la souris pendant un test. | utilisateur |
| D-14 | 2026-11-20 | Avant chaque test sur le PC, la session maître demande une halte à toutes les autres sessions Claude du PC, attend qu'elles soient au repos, puis leur rend la main après le test, même s'il échoue. | D'autres sessions tournent sur le PC : une action de l'une d'elles pendant le test fausserait le résultat. | utilisateur |
