# Sagouindupm | VIctory

**Une version de VIctory optimisée pour les petites configurations : profil Series S uniquement.**
*A VIctory build tuned for low-end PCs, Xbox Series S profile only.*

Ce dépôt part de [fxceless/victory-fork](https://github.com/fxceless/victory-fork), lui-même basé sur
[VIctory](https://github.com/Marvisak/VIctory), une couche de compatibilité qui lance un jeu Xbox Series
sur Windows à partir de fichiers que l'utilisateur possède. Les autres forks visent les grosses cartes
(Series X, 12 Go de VRAM ou plus). Ici on vise **une carte de 8 Go et un processeur milieu de gamme** et
on optimise tout pour ça.

**Ce dépôt ne contient aucun fichier, code ou asset du jeu.** Il faut sa propre copie. Projet sans lien
avec l'éditeur ou le développeur du jeu.

## Où on en est

Mesuré sur la machine de référence : **RTX 3050 8 Go, i5-12400F, 32 Go de RAM, Windows 11**, profil
`--console series-s`.

| | |
|---|---|
| Progression | Prologue terminé, chapitre I jusqu'à la mission en bateau de Jason |
| Images par seconde | 4 à 7 fps au calme (environ 210 à 230 ms par image), 1 à 2 fps en mer ouverte |
| Pires accrocs | 0,6 s (contre 2,4 s avant la reprise de #827 et #824) |
| VRAM | 7 à 8 Go sur 8 |
| Eau | Les bateaux et le joueur flottent (copie des cartes de vagues, `VICTORY_TEXTURE_BACK=1`) |
| Problèmes connus | Eau trop claire en surface, modèles basse définition au loin, taches de textures pas encore chargées, sauvegarde automatique parfois en échec |

La barre de titre de la fenêtre affiche en direct : fps, ms par image, nombre de dessins, carte
graphique, VRAM utilisée sur le budget, et nombre de shaders traduits.

## Ce que ce dépôt apporte

Nos propres travaux :
- **Second plan d'affichage de la Series S** (l'interface est dessinée par-dessus l'image du jeu) et
  agrandissement simple de l'image quand l'upscaler de la console n'est pas disponible.
- **Commandes clavier qui ne perdent plus les appuis** à quelques images par seconde.
- **Sauvegardes** : emplacements XGameSave manquants (enregistrement bloquant, énumération par préfixe).
- **Shaders** : lignes auxiliaires des pixels gardées hors des boucles, profondeur anticipée déclarée,
  vues de constantes limitées à 64 Kio (sinon NVIDIA retire le périphérique).
- **Passes d'écran** avec des fils de compilation dédiés.
- **Lanceurs Series S** réglés pour 8 Go (dossier [`series-s/`](series-s/)).

Repris d'autres projets, avec leurs auteurs d'origine conservés dans l'historique (`cherry-pick -x`) :
- **Geckomacabre** : les textures que le jeu relit sur le processeur (l'eau) sont recopiées en mémoire du jeu.
- **Belobug** : lumière ambiante minimale, ombrage par la profondeur, reflets limités à la lumière ambiante (eau blanche).
- **edsuuu** : traduction des shaders wave64 sur cartes 32 lignes, marques du tas.
- **fxceless** : copies en place, fils sur les cœurs rapides, journal d'images, réglages des fenêtres de dessin.
- **Nico Panu, UwUCammie, Lilith133T, Pilote Production, moe-miau, uiliannn** : correctifs mémoire, copies, pipelines, DirectStorage, sauvegardes.

## Lancer

1. Compiler : `cargo build --release` (chaîne Rust indiquée dans `rust-toolchain.toml`).
2. Copier `victory.example.toml` vers votre propre `victory.toml` et le renseigner.
3. Lancer `series-s\launch_series_s.bat` après y avoir mis vos chemins.

Commandes clavier : **G** bascule entre à pied et véhicule. À pied : ZQSD/WASD, souris pour la caméra
(clic pour la capturer, Échap pour la libérer). En véhicule ou en bateau : W ou clic gauche pour
accélérer, S ou clic droit pour freiner, A/D pour tourner, Maj gauche pour le frein à main.

## Contribuer

Personne ne pousse directement sur `main`. Tout passe par une pull request, relue et fusionnée par le
mainteneur. Voir [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

GPL-3.0-or-later, comme VIctory (voir [LICENSE](LICENSE)). Aucun code sous licence GPL-2.0-only
(par exemple XCL) n'est accepté ici.

## Crédits

- **[VIctory](https://github.com/Marvisak/VIctory)** de Marvisak et ses contributeurs : la base de tout
  (chargeur, services d'exécution, Direct3D 12, recompilateur de shaders).
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)** et sa communauté : la base de
  travail directe de ce dépôt.
- Les auteurs cités plus haut, dont le travail est repris avec leur historique.
- Développé avec l'aide de [Claude Code](https://claude.com/claude-code).
