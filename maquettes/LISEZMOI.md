# Maquettes du HUD : cinq variantes à choisir

Ouvre `index.html` (aucune connexion nécessaire) ou regarde les images `variante-1.png` à
`variante-5.png`. Chaque image montre la variante en six vues, à taille réelle pour un affichage
Windows à 100 %, sur des fonds simulés (bureau clair, jeu de nuit, jeu en plein jour) :
**a** arrêtée · **b** active, bouton gauche enfoncé · **c** capture d'un nouveau raccourci ·
**d** raccourci refusé · **e** trois macros, dont deux « À venir » · **f** forme repliée.

## Les cinq variantes

| N° | Nom | Idée | Force | Faiblesse |
|---|---|---|---|---|
| 1 | Cartes | Panneau vertical, une carte complète par macro ; active : la carte se borde de vert | Tout est visible et nommé, sans aucun clic | Haut (330 px pour une macro, 500 px à trois) ; chaque vraie macro ajoutera une carte complète |
| 2 | Liste compacte | Une ligne par macro (bouton ▶, état, touche) ; les réglages s'ouvrent sous la ligne | Le plus petit (288 × 84 px réglages fermés) ; active : toute la ligne verte, visible du coin de l'œil ; une macro de plus = une ligne de plus | Le bouton ▶ n'a pas de libellé écrit ; bouton maintenu et raccourci demandent un clic sur la flèche |
| 3 | Grand état | Thème clair ; grand bandeau d'état avec le bouton Activer dedans ; repliée : le bandeau seul | L'état se lit de loin, même replié | Le panneau blanc éblouit dans un jeu sombre et se détache moins sur un ciel clair ; haut (455 px à trois macros) |
| 4 | Souris | Une souris dessinée : on choisit le bouton sur le dessin, le bouton enfoncé s'allume en vert avec une flèche | Le choix du bouton se comprend sans lire | Le plus large (344 px) ; le dessin ne vaut que pour les macros de souris, pas pour une macro de clavier |
| 5 | Colonnes | Une colonne par macro, coiffée de sa grande touche de raccourci ; repliée : une rangée de touches | Le raccourci de chaque macro saute aux yeux ; la forme repliée à plusieurs macros est la plus parlante | Haut et étroit pour une macro (≈ 400 px) ; s'élargit à chaque macro (au-delà de trois, trop large) |

## Ma recommandation : la 2, « Liste compacte »

- C'est un HUD posé sur un jeu : la 2 cache le moins d'écran (une ligne de 50 px par macro) et se
  replie en une pastille de 34 px.
- L'état actif est le plus visible pour la place prise : toute la ligne passe au vert, avec le mot
  « Active » et le rond ●, donc lisible aussi sans la couleur.
- Elle grandit proprement : chaque nouvelle macro ajoute une ligne, avec sa touche alignée à droite
  ; on lit d'un coup « quelle touche fait quoi ».
- Ce qu'on change rarement (bouton maintenu, raccourci) est à un clic, ce qu'on regarde souvent
  (l'état) est toujours là.

Deuxième choix : la 1 « Cartes », si tu préfères tout voir sans jamais ouvrir de réglages. Les
mélanges sont possibles (par exemple la 2 avec la forme repliée « rangée de touches » de la 5), et
les couleurs claires ou sombres passent d'une variante à l'autre.

## Ce qui vaut pour les cinq

- Police **Inter** (libre, licence SIL Open Font License 1.1), embarquée dans la page et, plus
  tard, dans l'exécutable : la police par défaut d'egui est trop fine et n'a pas de gras.
- Tout est dessinable en egui : rectangles arrondis, couleurs pleines, contours, une ombre simple,
  texte et icônes simples (le dessin de souris de la 4 n'est fait que de formes simples).
- L'état n'est jamais dit par la couleur seule : toujours un mot et une forme (■ arrêtée, ● active,
  « À venir » écrit). Vérifié en regardant les cinq images passées en niveaux de gris.
- Contraste mesuré automatiquement sur chaque texte de chaque HUD : 5,4:1 au minimum (7,5:1 pour
  les variantes sombres) ; texte grisé « À venir » : 3,3:1 au minimum, lisible. Le bord du panneau
  (gris moyen cerclé de noir) se voit sur fond blanc comme sur fond noir.
- Boutons de 24 à 38 px de haut, jamais sous le minimum de 24 px.
- Les deux macros « À venir » (Clic répété, Touche maintenue) sont des exemples pour montrer la
  place prise : ni leur nom ni leurs options ne sont des décisions.
- Repliée, le HUD ne montre pas sa version : elle réapparaît en le dépliant.

## Refaire la page et les images

```bash
node maquettes/outils/generer.mjs   # écrit index.html
node maquettes/outils/rendre.mjs    # écrit les PNG et contrôle contraste et débordements
```
