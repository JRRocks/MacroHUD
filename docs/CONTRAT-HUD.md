# Contrat du HUD

Ce que le HUD montre et ce qu'il peut demander (D-11). La session « programmation » l'expose en
types Rust ; la session « visuel » dessine avec, et rien d'autre. S'il manque quelque chose : la
session maître tranche et complète ce fichier, personne d'autre.

## Ce que le HUD reçoit

- Le nom et la version du logiciel (« MacroHUD 0.1.7 »).
- La liste des macros, dans l'ordre. Pour chacune :
  - son nom (« Maintenir le clic ») et une description d'une ligne ;
  - son état : arrêtée, active, ou indisponible (avec la raison en une ligne) ;
  - son raccourci, en texte lisible (« F8 », « Ctrl+Alt+M ») ;
  - ses options ; pour « Maintenir le clic » : le bouton maintenu, parmi Gauche, Droit, Milieu ;
  - si elle est disponible (une macro « à venir » se montre grisée).
- La capture de raccourci en cours : quelle macro attend une touche, ou aucune.
- Un message ponctuel (raccourci enregistré, raccourci refusé, erreur), avec son niveau : info,
  avertissement ou erreur.
  - Ajout du 2026-11-20 (D-16) : la macro qu'il concerne, s'il en concerne une. Un raccourci refusé
    est un message de sorte « raccourci refusé », niveau avertissement, qui concerne la macro dont on
    changeait le raccourci, avec la touche refusée et un texte court sur le modèle de la maquette :
    « Alt+Tab est déjà pris par Windows. F8 reste actif. » Le HUD le montre comme la maquette :
    encadré ambre titré « Raccourci refusé », triangle, bouton « Choisir une autre touche ».
- Si le HUD est replié ou déplié.

## Ce que le HUD peut demander

- Activer ou arrêter une macro.
- Commencer la capture d'un nouveau raccourci pour une macro, ou l'annuler (Échap). Ajout du
  2026-11-20 : aussi depuis l'encadré « Raccourci refusé » (« Choisir une autre touche »), pour la
  macro que ce message concerne.
- Changer une option d'une macro (le bouton maintenu).
- Replier ou déplier le HUD ; le déplacer en le glissant (position retenue).
- Quitter MacroHUD (relâche d'abord tout ce qui est enfoncé).

## Comportements fixés, non visuels

- F8 bascule « Maintenir le clic » partout, même quand le HUD n'a pas le focus.
- Un clic physique sur le bouton maintenu arrête la macro ; le HUD passe à « arrêtée ».
- Le HUD reste au-dessus des autres fenêtres et ne prend pas le focus quand une macro bascule par
  son raccourci.
- Un raccourci déjà pris (par Windows ou par une autre macro) est refusé avec un message en
  français ; l'ancien raccourci reste actif.
- Les réglages (raccourcis, options, position, replié ou déplié) sont gardés dans
  `%APPDATA%\MacroHUD`.
