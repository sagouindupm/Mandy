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
descripteurs prend 60-150 ms sur un autre fil, bloquée par un verrou partagé avec les fils
d'enregistrement du jeu.

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
| Passe « masquée » des octets changés (`VICTORY_MASKED_MIN=64`, code présent, désactivé) | cold-project-vi aff5fb2 (porté) | 388 / 280 ms, +1,3 Go de VRAM ; l'attente passe à « title memory read » (75 ms) |
| Passe du tas par tranches (`VICTORY_HEAP_ROLL=1`, code présent, désactivé) | cold-project-vi 1141d35 | passe 110-170 ms au lieu de 63-150 ; 250 / 453 ms |
| Saut des shaders de ray tracing (`rt_off_hashes.txt` + `VICTORY_BVH_HOLD=1`) | trustno1-coder/VIctory-NVIDIA | 362 / 241 ms ; eau grise et verte avec des taches noires |
| `VICTORY_BVH_HOLD=1` seul | variable existante | 252 / 217 ms ; eau vert sombre et taches noires |
| `VICTORY_HEAP_SYNC_TABLES=0` | variable existante | 265 / 276 ms, pas de gain |
| `VICTORY_ASYNC_PRESENT=1` | variable existante | 272 / 294 contre 241 / 269 ms |
| TABLE_BUFFERS / SYNC_NULL_SKIP, DYNAMIC_SLOTS | uiliannn/VItoriaBr | plus lent ici |
| Cache de recherche (#1237) | fxceless | plus lent ici |
| Passe du tas en fils, mémoïsation, cache de clés | nous | plus lent (verrou partagé) |
| Formes wave64 « deep skips » coupées partout | — | pire ; seulement les 4 programmes de l'eau |
| DLSS ultra performance | — | ~4 % seulement, image floue |
| Builds entiers d'autres forks sur cette machine | Neptune, avatarmanz + réglages cool_fox, cold-project-vi | 290-450 ms, image moins bonne ou plantage |
| Reflets tenus (Belobug e633374b) | Belobug | eau toujours blanche |
| Constructeur BVH seul | — | plantage à 115 s |

## À essayer (par ordre d'intérêt)

Prenez une ligne, ouvrez une *issue* « [en cours] … » pour que personne ne fasse la même chose
(voir [CONTRIBUTING.md](../CONTRIBUTING.md)). Taille : S = quelques heures, M = une journée, L = plus.

| # | Quoi | Pourquoi | Pistes | Taille |
|---|---|---|---|---|
| 1 | **Verrous par emplacement du tas** (branche `perf/heap-locks`, en mesure) | la passe sur les buffers (60-150 ms) et les fils du jeu attendent le même verrou | d'après avatarmanz 6c17ce9 | fait, à mesurer |
| 2 | Ensembles d'emplacements en bits, passe du tas sur plusieurs fils | même verrou ; ~5 ms + parallélisme | avatarmanz c3e643e, 44629de (`shows_none`) | M |
| 3 | **Attente « title memory read »** (~40-50 ms par image) | une copie de buffer vers une zone que les shaders en vol écrivent attend la carte | `host.rs blocked_by` : faire passer la copie dans la file des vols au lieu d'attendre | M-L |
| 4 | Recopie vers le jeu (~60 ms) | sur le chemin critique | profil (`VICTORY_PROFILE=1`, PDB à côté de l'exe) ; comparaisons sur la carte | M |
| 5 | Préparation CPU (~100 ms) | le plus gros poste | profil ; `TablePlan` (codec_d e05d463) | L |
| 6 | Visages noirs par moments | image | valeurs 16 bits lues comme flottants sur NVIDIA (trustno1 77e13a97, fxceless PR 927) | M |
| 7 | Soleil noir, ciel en bandes | image | `VICTORY_NAN_SCRUB` (Ranxanat0r/cool_fox-dlss a51ae6d, ~40 lignes) | S |
| 8 | Buffers périmés avec `UNWATCHED_FRAMES=2` | image | codec_d 18bdf79 (balayage par tas, plage agrandie) | S |
| 9 | Eau peu profonde blanche, rectangles fantômes sur l'eau | image | `docs/deserialize-pipelines.md`, captures `VICTORY_CAPTURE_AT` | M |
| 10 | Variables à mesurer | sans code | `VICTORY_WIDE_REFRESH=1` (1 passage : 221 ms), `VICTORY_TEXTURE_STAGE_MS=24 VICTORY_WINDOW_KB=1024 VICTORY_WINDOW_TAILS=1` (1 passage : 217 ms, p90 383 ms au lieu de ~500 : à confirmer), `VICTORY_JOIN_COPIES=1`, `VICTORY_INDIRECT_WINDOW=1`, `VICTORY_CARD_REHASH_RUNS=1` | S |
| 11 | Mémoire de la carte écrite directement (Resizable BAR) | -2 Go de mémoire partagée | cold-project-vi 0030cd4 | M-L |
| 12 | **Autres machines** | on ne connaît que la RTX 3050 | mesures sur AMD, 12 Go, autres processeurs : très utiles | S |
