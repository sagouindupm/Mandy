# Sagouindupm | VIctory

**Une version de VIctory optimisée pour les petites configurations : profil Series S uniquement.**
*A VIctory build tuned for low-end PCs, Xbox Series S profile only.*

Depuis le 27/11/2026, ce dépôt part de la lignée la plus rapide sur les cartes NVIDIA modestes :
[Maykzera/victory-rtx3060](https://github.com/Maykzera/victory-rtx3060) (réglages pour RTX 3060 et
processeur 6 cœurs), lui-même fork de [trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA),
de [avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX), de
[fxceless/victory-fork](https://github.com/fxceless/victory-fork) et de [VIctory](https://github.com/Marvisak/VIctory).
On y ajoute nos correctifs, nos lanceurs Series S et nos mesures. Le module de chargement dérivé de XCL
(GPL-2.0 seulement) en est retiré et remplacé par notre propre émulation des instructions AMD.

**Ce dépôt ne contient aucun fichier, code ou asset du jeu.** Il faut sa propre copie. Projet sans lien
avec l'éditeur ou le développeur du jeu.

## Où on en est (27/11/2026)

Machine de référence : **RTX 3050 8 Go, i5-12400F, 32 Go de RAM, Windows 11**, `--console series-s`,
540p + upscaler du jeu, mission du bateau (chapitre I), shaders et pipelines préparés.

| | ms par image | fps |
|---|---|---|
| Ancienne base (fxceless + nos correctifs, 26/11) | 213-322 | 3-4,7 |
| **Nouvelle base (lignée trustno1/Maykzera), cinématique puis bateau en jeu** | **98-103 (p90 132)** | **~10** |

Image : eau réaliste en cinématique et en jeu (vagues, reflets du soleil), visages détaillés, interface
Series S. Le détail des essais (gardés, rejetés, à faire) est dans **[docs/essais.md](docs/essais.md)**.

## Lancer

### Débutant : le lanceur graphique

Double-cliquer sur **`series-s\Sagouindupm-Launcher.bat`**. La première fois, il construit la fenêtre du
lanceur (dossier `launcher\`, 5 à 10 minutes) : il faut Node.js, Rust et les outils C++ de Microsoft, et le
fichier propose de les installer avec winget s'ils manquent. Ensuite la fenêtre s'ouvre directement.

1. **Préparation** → **« Tout préparer »** : compile VIctory, trouve le jeu (`game_scarlett_master.exe` de
   votre copie) et traduit les shaders (15-30 min, une seule fois). Six étapes, chacune avec son état et son
   bouton : moteur, jeu, shaders, pipelines, cache de shaders NVIDIA, alimentation ; en dessous, « Ce PC »
   (carte, RAM, mémoire virtuelle, Resizable BAR).
2. **Jouer** → **JOUER**. Le profil (Recommandé ou un mode DLSS) se choisit à gauche. Cette première partie
   note les pipelines que le jeu crée, et une capture d'écran est prise au bout de 3 minutes : vos captures
   (`%LOCALAPPDATA%\Sagouindupm\captures`) deviennent le fond de l'accueil.
3. Après la partie : **Préparation** → étape 4, **« Construire »** (quelques minutes). La suivante n'a plus
   d'à-coups ni de dessins manquants : c'est ce qui fait passer de ~5 à ~10 fps ici.

Pour le construire à la main : `cd launcher`, `npm ci`, `npm run tauri build -- --no-bundle`
(résultat : `launcher\src-tauri\target\release\sagouindupm-launcher.exe`).

### Utilisateurs avancés : le lanceur console

1. Compiler : `cargo build --release --locked -p victory` (chaîne Rust de `rust-toolchain.toml`, outils
   MSVC et Windows SDK).
2. Double-cliquer sur **`series-s\Sagouindupm.bat`**. Au premier lancement il demande où est
   `game_scarlett_master.exe` (votre copie du jeu).
3. Menu, dans l'ordre la première fois :
   - **[2] Préparer les shaders** : traduit tous les shaders une fois (15-30 min). Indispensable.
   - **[1] Jouer** : chaque partie note les pipelines que le jeu crée.
   - **[3] Construire les pipelines** (quelques minutes) : la partie suivante n'a plus d'à-coups ni de
     dessins manquants.

Les deux lanceurs lisent les mêmes réglages mesurés, dans **`series-s\reglages.json`**, et la même
configuration (`%LOCALAPPDATA%\Sagouindupm`). Les scripts de mesure (`scripts\bench`) appellent `victory.exe`
directement.

**Cartes NVIDIA** : réglez la **taille du cache de shaders sur « Illimitée »** (Panneau de configuration
NVIDIA → Gérer les paramètres 3D). PC à moins de 24 Go de RAM : le lanceur place tout seul une partie de la
mémoire du jeu dans la carte (`VICTORY_WINDOWS_BAR_MB=3072`, il faut le Resizable BAR).

## Contribuer

Toute aide est la bienvenue, en particulier des **mesures sur d'autres machines**.
- Humains : [CONTRIBUTING.md](CONTRIBUTING.md). Assistants IA : [AGENTS.md](AGENTS.md).
- Personne ne pousse directement sur `main` : tout passe par une pull request.

## Contributeurs

| Qui | Contribution |
|---|---|
| [@sagouindupm](https://github.com/sagouindupm) | mainteneur |
| [@Xiaoxi2021](https://github.com/Xiaoxi2021) | mesures RTX 5070 (#9) |
| [@lucid29a](https://github.com/lucid29a) | mesures RTX 4070 Laptop (#10) |
| [@Tejvardhan-717](https://github.com/Tejvardhan-717) | verrou du tas de descripteurs (#13, #15) |
| [@Liovera](https://github.com/Liovera) | mesures RTX 4050 Laptop 16 Go (#14) |
| [@LuizFernandoMoretto](https://github.com/LuizFernandoMoretto) | politique du dépôt, rapport de tests (#16) |

## Licence

GPL-3.0-or-later (voir [LICENSE](LICENSE)). Aucun code sous licence GPL-2.0-only (XCL par exemple).

## Crédits

- **[VIctory](https://github.com/Marvisak/VIctory)** de Marvisak et ses contributeurs : la base de tout.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**, **[avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX)**
  (architecture rapide : fils, relais d'écriture, copies sur la carte), **[trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA)**
  (NVIDIA, interface Series S, visages), **[Maykzera/victory-rtx3060](https://github.com/Maykzera/victory-rtx3060)**
  (réglages 6 cœurs, cache de shaders valable sur NVIDIA, touches scriptées). Leur README est conservé dans
  [docs/README-victory-nvidia-rtx3060.md](docs/README-victory-nvidia-rtx3060.md) avec la liste complète de
  leurs sources.
- **[WASDeKeys/victory-low-end-pc](https://github.com/WASDeKeys/victory-low-end-pc)**, **[stellarr-studio/kelvaroq](https://github.com/stellarr-studio/kelvaroq)** :
  mesures et méthodes pour petites cartes.
- Développé avec l'aide de [Claude Code](https://claude.com/claude-code).
