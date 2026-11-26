# Contribuer à Sagouindupm | VIctory

*English summary at the bottom.*

Le but est unique : **faire tourner le jeu au mieux sur une petite configuration, profil Series S**
(carte de 8 Go, processeur milieu de gamme), avec une image aussi proche que possible de la console.
Aujourd'hui : ~4-5 images par seconde sur RTX 3050. Objectif : 15.

## Démarrer en 10 minutes

1. Lisez [docs/essais.md](docs/essais.md) : ce qui a été mesuré, gardé, rejeté, et la liste
   **« À essayer »**.
2. Si vous travaillez avec une IA, donnez-lui d'abord [AGENTS.md](AGENTS.md).
3. Compilez : `cargo build --release -p victory` (chaîne Rust dans `rust-toolchain.toml`, outils
   MSVC et Windows SDK).
4. Lancez avec `series-s\Sagouindupm.bat` (ou `series-s\launch_series_s.bat`).
5. **Cartes NVIDIA** : Panneau de configuration NVIDIA → Gérer les paramètres 3D → **Taille du cache
   de shaders → Illimitée**. Sinon, une fois les 4 Go par défaut remplis, chaque nouvelle version
   recompile ses shaders à chaque lancement et paraît bien plus lente (voir `docs/essais.md`).
6. Préparez votre banc de mesure : [scripts/bench/run.sh](scripts/bench/run.sh) (variables à
   renseigner en tête du fichier) et une copie de votre sauvegarde à l'endroit mesuré.

## Se répartir le travail (personne ne se marche dessus)

1. **Une tâche = une *issue*.** Avant de commencer, ouvrez une issue (ou commentez celle qui
   existe) avec le titre `[en cours] <la tâche>` et la ligne de `docs/essais.md` concernée.
   Si quelqu'un l'a déjà prise, choisissez autre chose, ou proposez-lui de vous partager le travail
   dans l'issue.
2. **Une personne par zone à la fois** : deux PR qui touchent le même gros fichier se gênent.
   Les zones :

   | Zone | Fichiers |
   |---|---|
   | Soumission, vols, recopie, attentes | `crates/victory-gpu/src/host.rs`, `queue.rs`, `queue2.rs` |
   | Tas de descripteurs, passe sur les buffers | `crates/victory-gpu/src/descriptor/host.rs`, `draw.rs` |
   | Copies de la mémoire du jeu | `crates/victory-gpu/src/shadow.rs`, `plaincopy.rs`, `watch.rs` |
   | Textures | `crates/victory-gpu/src/texture.rs`, `resource.rs` |
   | Traduction des shaders | `crates/victory-shader/` |
   | Services Xbox, entrées, sauvegardes | `crates/victory-hle/` |
   | Lanceurs et réglages | `series-s/` |
   | Mesures sur d'autres machines | `docs/essais.md` seulement |

3. **Une issue sans nouvelles depuis 3 jours est libre.** Écrivez où vous en êtes, même pour dire
   « abandonné, voici pourquoi » : c'est un résultat.
4. **Branches** : `perf/…` (vitesse), `fix/…` (image ou plantage), `re/…` (rétro-ingénierie),
   `docs/…`. Partez toujours de `main` à jour.

## Règles

1. **Pas de poussée directe sur `main`.** Une pull request depuis une branche ou un fork, relue et
   fusionnée par le mainteneur uniquement.
2. **Une pull request = un sujet**, avec un titre qui dit ce que ça change.
3. **Des mesures**, faites comme dans [docs/essais.md](docs/essais.md) : médiane avant/après sur au
   moins deux passages alternés, la machine (carte, processeur, RAM), l'endroit du jeu. Pour un
   correctif d'image : une capture avant/après.
4. **Le résultat va dans `docs/essais.md`** dans la même PR : ligne dans « Gardé » ou « Rejeté », et
   mise à jour de « À essayer ». Un essai raté documenté vaut une PR.
5. **Un réglage nouveau = une variable `VICTORY_…`**, documentée dans un commentaire là où elle est
   lue, et désactivée par défaut tant qu'elle n'est pas mesurée.
6. **Rien du jeu** : ni fichiers, ni dumps, ni journaux bruts, ni captures de code du jeu. Pas de
   patch qui modifie l'exécutable du jeu sur le disque.
7. **Licence** : GPL-3.0-or-later. Aucun code GPL-2.0-only (XCL, Mandy d'origine) ni sous licence
   incompatible.
8. **Crédits** : si vous reprenez le travail de quelqu'un, gardez son commit et son nom
   (`git cherry-pick -x`) ou nommez-le dans le message (`d'après <auteur> <commit>`). Ne
   réattribuez jamais le travail d'un autre.
9. **Contributeurs** : ajoutez-vous au tableau « Contributeurs » du README dans votre PR (pseudo GitHub et ce
   que vous avez apporté). Une mesure ou un rapport de bug compte aussi.
10. **IA** : bienvenue, à signaler dans la description de la PR (« AI-assisted: … »).

## Avant d'ouvrir la PR

- `cargo build --release -p victory` passe.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` et `cargo test -p <crate touché>`
  passent.
- Le jeu démarre en `--console series-s` et atteint le monde.
- `docs/essais.md` est à jour.

---

## English summary

Goal: the best possible game on a **low-end PC, Series S profile** (8 GB card, mid-range CPU), as
close to the console's picture as we can. Today ~4-5 fps on an RTX 3050; target 15.

- Read [docs/essais.md](docs/essais.md) first (what was measured, kept, rejected, and the **to-try**
  list). Give [AGENTS.md](AGENTS.md) to your AI assistant.
- **Claim before you start**: open an issue titled `[in progress] <task>`. One person per code area
  at a time (table above). Issues silent for 3 days are free again.
- PRs only, one topic each, with measurements (median of at least two alternating runs from the same
  save, your hardware) and a row added to `docs/essais.md`, negative results included.
- New behaviour goes behind a documented `VICTORY_…` switch, off until measured.
- No game files, GPL-3.0-or-later only, keep original authors (`cherry-pick -x`), say when an AI
  helped.
