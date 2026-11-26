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

## Où on en est (26/11/2026)

Mesuré sur la machine de référence : **RTX 3050 8 Go, i5-12400F, 32 Go de RAM, Windows 11**, profil
`--console series-s`, DLSS mode performance, mission du bateau (chapitre I).

| | |
|---|---|
| Progression | Prologue terminé, chapitre I jusqu'à la mission en bateau |
| Images par seconde | **4 à 5 fps** dans le monde avec DLSS performance (médiane 206-248 ms par image ; 474 ms la veille) ; sans DLSS, compter ~20 % de moins |
| VRAM | ~5 Go sur 8 |
| Eau | Turquoise, les bateaux et le joueur flottent |
| Problèmes connus | Eau peu profonde blanche par endroits, visages noirs par moments, rectangles fantômes sur l'eau, textures qui arrivent en retard |

Le détail de chaque essai (gardé ou rejeté) et la liste de ce qu'il reste à essayer sont dans
**[docs/essais.md](docs/essais.md)**.

La barre de titre de la fenêtre affiche en direct : fps, ms par image, nombre de dessins, carte
graphique, VRAM utilisée sur le budget, et nombre de shaders traduits.

## Lancer

1. Compiler : `cargo build --release -p victory` (chaîne Rust indiquée dans `rust-toolchain.toml`,
   outils MSVC et Windows SDK).
2. Double-cliquer sur **`series-s\Sagouindupm.bat`**. Au premier lancement, il demande où est
   `game_scarlett_master.exe` (votre copie du jeu) et écrit la configuration tout seul.
3. Menu : **Jouer**, **Préparer les shaders à l'avance** (recommandé la première fois : évite les
   écrans noirs et les à-coups), **Copier mes sauvegardes**.

Tant que les shaders ne sont pas préparés, le lanceur demande au moteur d'attendre un shader
(jusqu'à 3 s) au lieu de sauter le dessin : quelques ralentissements, pas d'écran noir.
Pour les connaisseurs, `series-s\launch_series_s.bat` lance directement avec les mêmes réglages
(DLSS en option, voir le fichier).

**DLSS (cartes NVIDIA RTX), fortement conseillé** : nos chiffres sont mesurés avec DLSS en mode
performance (le jeu calcule en 960x540, DLSS remet en 1080p : environ -20 % par image et une image plus
nette). Téléchargez le SDK Streamline de NVIDIA (https://github.com/NVIDIAGameWorks/Streamline, page
*Releases*), et copiez les 4 fichiers de `bin\x64` (`sl.interposer.dll`, `sl.common.dll`,
`sl.dlss.dll`, `nvngx_dlss.dll`) dans un dossier **`streamline\`** à la racine du dépôt. Les lanceurs
l'activent tout seuls quand ils les trouvent.

**Préparez les shaders** (menu du lanceur) avant de mesurer : sinon le jeu attend chaque nouveau shader
(pics de plusieurs secondes les premières minutes).

**Cartes NVIDIA** : réglez la **taille du cache de shaders sur « Illimitée »** (Panneau de configuration
NVIDIA → Gérer les paramètres 3D). Avec la limite par défaut de 4 Go, le cache se remplit et le jeu
recompile ses shaders à chaque lancement (jusqu'à 2 fois plus lent). Les 2-3 premiers lancements d'une
nouvelle version sont plus lents : le pilote remplit son cache.

Commandes clavier : **G** bascule entre à pied et véhicule. À pied : ZQSD/WASD, souris pour la caméra
(**clic pour la capturer, Alt ou Échap pour la libérer**), X pour marcher. En véhicule ou en bateau : W ou clic gauche
pour accélérer, S ou clic droit pour freiner, A/D pour tourner, Maj gauche pour le frein à main.

## Ce que ce dépôt apporte

Nos propres travaux :
- **Second plan d'affichage de la Series S** (l'interface est dessinée par-dessus l'image du jeu) et
  agrandissement simple de l'image quand l'upscaler de la console n'est pas disponible.
- **DLSS** (mode performance) pour les cartes NVIDIA.
- **Eau turquoise sur NVIDIA** : les programmes de calcul de l'eau gardés hors de la forme wave64
  « sans sauts profonds ».
- **Streaming** : les pages de mémoire mappées par lots sous un seul verrou (-17 % par image).
- **Moins d'attentes de la carte** : octets changés sous les écritures des shaders envoyés groupés,
  copies plus grandes gardées sur la carte, mémoire non surveillée recopiée une image sur deux.
- **Commandes clavier qui ne perdent plus les appuis** à quelques images par seconde.
- **Sauvegardes** : emplacements XGameSave manquants (enregistrement bloquant, énumération par préfixe).
- **Shaders** : lignes auxiliaires des pixels gardées hors des boucles, profondeur anticipée déclarée,
  vues de constantes limitées à 64 Kio (sinon NVIDIA retire le périphérique).
- **Lanceurs Series S** réglés pour 8 Go (dossier [`series-s/`](series-s/)) et **banc de mesure**
  ([`scripts/bench/`](scripts/bench/)).

Repris d'autres projets, avec leurs auteurs d'origine conservés (`cherry-pick -x` ou nommés dans le
message) :
- **avatarmanz** (VIctory-7900XTX) : cibles de rendu et pipelines non redéfinis.
- **trustno1-coder** (VIctory-NVIDIA) : copies sur la carte jusqu'à 1,5 Go.
- **edupode / GextFRDD** (cold-project-vi) : capture de la souris, idées sur les attentes de la carte.
- **Geckomacabre** : les textures que le jeu relit sur le processeur (l'eau) sont recopiées en mémoire du jeu.
- **Belobug** : lumière ambiante minimale, ombrage par la profondeur, reflets limités à la lumière ambiante.
- **edsuuu** : traduction des shaders wave64 sur cartes 32 lignes, marques du tas.
- **fxceless** : copies en place, fils sur les cœurs rapides, journal d'images, réglages des fenêtres de dessin.
- **Nico Panu, UwUCammie, Lilith133T, Pilote Production, moe-miau, uiliannn** : correctifs mémoire, copies, pipelines, DirectStorage, sauvegardes.

## Contribuer

Toute aide est la bienvenue, en particulier des **mesures sur d'autres machines** et les pistes de la
liste « À essayer » de [docs/essais.md](docs/essais.md).
- Humains : [CONTRIBUTING.md](CONTRIBUTING.md) (comment se répartir le travail, mesurer, proposer).
- Assistants IA : [AGENTS.md](AGENTS.md) (le projet, la carte du code, les règles, où aider).

Personne ne pousse directement sur `main` : tout passe par une pull request, relue et fusionnée par le
mainteneur.

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
