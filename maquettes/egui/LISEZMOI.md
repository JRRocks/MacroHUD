# Crate d'essai du HUD (session « visuel »)

Le HUD retenu (variante 2 « Liste compacte », D-12) est codé dans `src/hud/` à la racine du dépôt.
Le crate principal n'existe pas encore : ce petit crate compile `src/hud/` tel quel et le dessine
en PNG, sans écran ni carte graphique.

## Ce qu'on regarde

- `rendus/variante-2-egui.png` : les six vues de la maquette (a à f), dessinées par le vrai code du
  HUD, sur les mêmes fonds et dans la même mise en page que `maquettes/variante-2.png`.
- `rendus/hors-maquette-egui.png` : trois cas du contrat que la maquette ne montrait pas (macro
  indisponible, message d'information, message d'erreur), à valider.
- `rendus/vue-*.png` : chaque vue seule, fond transparent, à 100 % et à 200 % (`@2x`).

## Refaire les images et vérifier

```bash
cd maquettes/egui
cargo test                                  # écrit rendus/ et vérifie tailles, pixels, clics
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Les tests vérifient que chaque vue a exactement la taille de la maquette, que la ligne d'une macro
active est bien verte, et que de vrais clics (événements souris envoyés à egui) produisent les
demandes du contrat : activer, arrêter, changer de bouton, changer de raccourci, annuler, replier,
déplier, quitter. Une macro « À venir » ne réagit à aucun clic.

## Comment l'image est faite

egui produit des triangles ; `src/hud/render.rs` les remplit en logiciel comme le font ses
moteurs graphiques (alpha prémultiplié, mélange en espace gamma), puis écrit le PNG. Le rendu à
l'écran sous Windows peut différer d'un pixel sur le lissage du texte.

Les fonds (`fonds/*.png`) viennent des scènes de la page des maquettes :
`node maquettes/outils/rendre.mjs --fonds maquettes/egui/fonds`.
