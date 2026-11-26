# Journal des essais / Test log

Tout ce qui a été mesuré, gardé ou rejeté, et ce qui reste à essayer. **Avant de commencer quelque
chose, regardez ici** : un essai déjà fait (même raté) fait gagner des heures. **Après un essai,
ajoutez une ligne**, même si le résultat est négatif.

*Everything measured, kept or rejected, and what is left to try. Read this before you start; add a
row after every test, negative results included.*

## Comment on mesure

- Machine de référence : **RTX 3050 8 Go, i5-12400F, 32 Go de RAM, Windows 11**, `--console series-s`,
  DLSS mode performance (`VICTORY_DLSS_MODE=performance`), réglages de `series-s/launch_series_s.bat`.
- Même endroit à chaque fois : la **mission du bateau** (chapitre I), à partir d'une **copie de la
  sauvegarde remise avant chaque passage** (le jeu sauvegarde tout seul : sans ça chaque passage part
  d'ailleurs).
- `scripts/bench/run.sh` : 5 minutes, médiane et p90 des images « monde » (300 dessins ou plus),
  RAM, VRAM, trois captures. **Un passage = ±100 ms de bruit** : on alterne A1 B1 A2 B2 au minimum.
- Ce qu'on lit dans le journal du jeu, ligne `... fps, ... ms/frame (X preparing ...; Y on the GPU;
  Z copying back ...)` et ligne `of the waiting for the card in settles, per frame, by what settled`.

## ⚠️ Cache de shaders du pilote NVIDIA : à régler avant de mesurer

Le pilote NVIDIA garde les pipelines qu'il a compilés dans `%LOCALAPPDATA%\NVIDIA\DXCache`,
**4 Go par application par défaut**. VIctory en compile des milliers : le cache est plein en
quelques jours de tests, et **tout exe compilé ensuite recompile ses pipelines à chaque lancement**
(300-500 ms par image au lieu de ~210, pour un code identique). Panneau de configuration NVIDIA →
Gérer les paramètres 3D → **Taille du cache de shaders → Illimitée** (ou 100 Go). Un exe nouveau
(ou déplacé) a aussi besoin de 2-3 lancements pour remplir son cache : comparez des builds
**chauffés**, jamais un nouvel exe contre un ancien.

Conséquence ici : le cache était plein depuis le 26/11 à 05h25. Les essais marqués **(à remesurer)**
ci-dessous comparaient un exe compilé après cette heure à l'ancien exe en cache : leur verdict ne
vaut rien tant qu'ils ne sont pas refaits. Les essais de **variables** (même exe) restent valables.

## Où on en est (26/11/2026)

| | ms par image (médiane) | fps |
|---|---|---|
| Base `feat/render` (25/11) | ~474 | 2,1 |
| + lot de mappage (#4) | ~394 | 2,5 |
| + cibles/pipelines non redéfinies (#6, avatarmanz) + `UNWATCHED_FRAMES=2` | ~250-280 | 3,6-4,0 |
| + `MERGE_GAP=16` + `CARD_BUFFERS_MB=1536` (26/11) | **~206-248** | **4,0-4,9** |

Où part une image aujourd'hui (~210 ms) : **préparation CPU ~100-125 ms**, GPU 30-100 ms, recopie
vers la mémoire du jeu ~60 ms, plus ~40-50 ms d'attente de la carte pour une copie de buffer
(« title memory read »). En parallèle, la passe sur les emplacements de buffers du tas de
descripteurs prend 60-150 ms sur un autre fil (ni un verrou plus fin ni des fils en plus ne l'ont
accélérée : à profiler).

### Ce que dit le profil (26/11, build diagnostics, `VICTORY_PROFILE=1`)

- **Le processeur n'est pas plein** : ~2,4 fils occupés sur 12. Une image est limitée par des
  **attentes en série** et des verrous, pas par le calcul.
- Le fil qui présente l'image attend **~37 ms par image** le verrou `RELEASING` (`queue.rs
  release_held_now`), tenu par le fil de soumission ; celui-ci y attend lui-même **~15 ms** que
  la recopie des vols soit faite (`host.rs relay_wait`, les « settles »).
- `HostHeap::mirror` crée des ressources du pilote (`CreateCommittedResource`, ~5 ms) **en tenant
  le verrou du tas** : les autres attendent ~10 ms. Un verrou plus fin ne sert à rien tant que la
  création se fait dessous.
- `texback::Target::new` attend ~10 ms le verrou `MAPPING` pendant les mappages de pages
  (`copy_page_mappings`, ~11 ms de mappage/démappage par image).
- Le fil de soumission hache et recopie des textures (`blake3` ~5 ms, copies gardées ~4 ms,
  `Staging::refresh` ~4 ms), peut-être inchangées.

### Implémenté, résultat en jeu à mesurer

| Essai | Changement | Vérification | Résultat de performance |
|---|---|---|---|
| #13, création des ressources hors du verrou `made` | `HostHeap::mirror` libère le verrou de comptabilité pendant la résolution/création D3D12; un verrou distinct protège les mutations des emplacements statiques. Le chemin de réfection des vues et la libération d'un emplacement prennent le même verrou. | `cargo test -p victory-gpu --lib` : 218 réussis, 6 ignorés; `cargo build --release -p victory` réussi; clippy workspace réussi. | **À mesurer** avec A1/B1/A2/B2 et une sauvegarde restaurée. Aucun gain de FPS n'est encore revendiqué. |

## Gardé

| Essai | Origine | Résultat | Où |
|---|---|---|---|
| Lot de mappage de pages sous un seul verrou | nous | 474 → 394 ms (-17 %) | `xmem.rs map_aliases` (#4) |
| Cibles de rendu et pipelines non redéfinis | avatarmanz ca1d519f, ab86ef7f | -12 % | #6 |
| `VICTORY_UNWATCHED_FRAMES=2` | nous | -17 % | lanceurs |
| Eau turquoise : `VICTORY_WAVE_DEEP_SKIPS_NOT=<4 programmes>` | nous (analyse des programmes de l'eau) | eau correcte, vitesse égale | lanceurs |
| `VICTORY_MERGE_GAP=16` + `VICTORY_MERGE_RUNS=65536` | variable existante | 233 / 216 ms contre ~280 ; attente « title memory changed under shader writes » 31 → 0 ms ; image vérifiée | lanceurs |
| `VICTORY_CARD_BUFFERS_MB=1536` (au lieu de 512) | trustno1-coder/VIctory-NVIDIA | 231 / 236 ms contre ~280, VRAM identique, p90 plus bas | lanceurs |
| DLSS mode performance | nous | 351 → 292 ms, image plus nette | `VICTORY_DLSS_MODE` |
| Souris : clic = capture, Alt/Échap = libère (ne bascule plus) | cold-project-vi | confort | `gameinput.rs` |
| `VICTORY_SKIP_PIPELINES` utilisable hors build diagnostics | nous | outil de test | `pipeline.rs` |

## Rejeté (mesuré, ne pas refaire sans idée nouvelle)

| Essai | Origine | Résultat |
|---|---|---|
| `VICTORY_MERGE_RUNS=65536` seul | — | 290 / 343 ms : l'attente se déplace vers « title memory read » |
| Passe « masquée » des octets changés (`VICTORY_MASKED_MIN=64`, code présent, désactivé) **(à remesurer)** | cold-project-vi aff5fb2 (porté) | 388 / 280 ms, +1,3 Go de VRAM ; l'attente passe à « title memory read » (75 ms) |
| Passe du tas par tranches (`VICTORY_HEAP_ROLL=1`, code présent, désactivé) **(à remesurer)** | cold-project-vi 1141d35 | passe 110-170 ms au lieu de 63-150 ; 250 / 453 ms |
| Saut des shaders de ray tracing (`rt_off_hashes.txt` + `VICTORY_BVH_HOLD=1`) (vitesse à remesurer ; l'eau abîmée reste vraie) | trustno1-coder/VIctory-NVIDIA | 362 / 241 ms ; eau grise et verte avec des taches noires |
| `VICTORY_BVH_HOLD=1` seul | variable existante | 252 / 217 ms ; eau vert sombre et taches noires |
| `VICTORY_HEAP_SYNC_TABLES=0` | variable existante | 265 / 276 ms, pas de gain |
| `VICTORY_ASYNC_PRESENT=1` | variable existante | 272 / 294 contre 241 / 269 ms |
| TABLE_BUFFERS / SYNC_NULL_SKIP, DYNAMIC_SLOTS | uiliannn/VItoriaBr | plus lent ici |
| Cache de recherche (#1237) | fxceless | plus lent ici |
| Passe du tas en fils, mémoïsation, cache de clés | nous | plus lent (verrou partagé) |
| Tas sous 512 verrous + chemin rapide sans verrou (branche `perf/heap-locks`) **(à remesurer)** | d'après avatarmanz 6c17ce9 | passe du tas 144-155 ms au lieu de 97 ; 440 / 263 ms contre 213 : le verrou n'est pas le goulot |
| Formes wave64 « deep skips » coupées partout | — | pire ; seulement les 4 programmes de l'eau |
| DLSS ultra performance | — | ~4 % seulement, image floue |
| Builds entiers d'autres forks sur cette machine **(à remesurer : nouveaux exe)** | Neptune, avatarmanz + réglages cool_fox, cold-project-vi | 290-450 ms, image moins bonne ou plantage |
| Reflets tenus (Belobug e633374b) | Belobug | eau toujours blanche |
| Constructeur BVH seul | — | plantage à 115 s |

## À essayer (par ordre d'intérêt)

Prenez une ligne, ouvrez une *issue* « [en cours] … » pour que personne ne fasse la même chose
(voir [CONTRIBUTING.md](../CONTRIBUTING.md)). Taille : S = quelques heures, M = une journée, L = plus.

| # | Quoi | Pourquoi | Pistes | Taille |
|---|---|---|---|---|
| 1 | Passe du tas : comprendre où partent ses 100-150 ms | ni le verrou ni les fils n'ont aidé ; profiler d'abord | build diagnostics + `VICTORY_PROFILE=1` ; avatarmanz 44629de (`shows_none` : 32 000 emplacements vides sur 41 000 sautés) | M |
| 3 | **Attente « title memory read »** (~40-50 ms par image) | une copie de buffer vers une zone que les shaders en vol écrivent attend la carte | `host.rs blocked_by` : faire passer la copie dans la file des vols au lieu d'attendre | M-L |
| 3b | ~~Créer les ressources hors du verrou du tas~~ (implémenté en #13; mesure en jeu encore requise) | ~10 ms d'attente + 5 ms de création observés avant le changement | `HostHeap::mirror` : libérer le verrou de comptabilité pendant la création, puis publier; comparer A1/B1/A2/B2 | M |
| 3c | `texback::Target::new` sans attendre `MAPPING` | ~10 ms sur les fils d'enregistrement | lire sans le verrou si possible, ou relâcher le verrou entre les lots de `copy_page_mappings` | S-M |
| 3d | Ne pas hacher/recopier les textures non écrites | ~5-10 ms sur le fil de soumission | regarder la surveillance d'écriture avant `blake3` | S-M |
| 4 | Recopie vers le jeu (~60 ms) | sur le chemin critique | profil (`VICTORY_PROFILE=1`, PDB à côté de l'exe) ; comparaisons sur la carte | M |
| 5 | Préparation CPU (~100 ms) | le plus gros poste | profil ; `TablePlan` (codec_d e05d463) | L |
| 6 | Visages noirs par moments | image | valeurs 16 bits lues comme flottants sur NVIDIA (trustno1 77e13a97, fxceless PR 927) | M |
| 7 | Soleil noir, ciel en bandes | image | `VICTORY_NAN_SCRUB` (Ranxanat0r/cool_fox-dlss a51ae6d, ~40 lignes) | S |
| 8 | Buffers périmés avec `UNWATCHED_FRAMES=2` | image | codec_d 18bdf79 (balayage par tas, plage agrandie) | S |
| 9 | Eau peu profonde blanche, rectangles fantômes sur l'eau | image | `docs/deserialize-pipelines.md`, captures `VICTORY_CAPTURE_AT` | M |
| 10 | Variables à mesurer | sans code | `VICTORY_WIDE_REFRESH=1` (1 passage : 221 ms), `VICTORY_TEXTURE_STAGE_MS=24 VICTORY_WINDOW_KB=1024 VICTORY_WINDOW_TAILS=1` (1 passage : 217 ms, p90 383 ms au lieu de ~500 : à confirmer), `VICTORY_JOIN_COPIES=1`, `VICTORY_INDIRECT_WINDOW=1`, `VICTORY_CARD_REHASH_RUNS=1` | S |
| 11 | Mémoire de la carte écrite directement (Resizable BAR) | -2 Go de mémoire partagée | cold-project-vi 0030cd4 | M-L |
| 12 | **Autres machines** | on ne connaît que la RTX 3050 | mesures sur AMD, 12 Go, autres processeurs : très utiles | S |
