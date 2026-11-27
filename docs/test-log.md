# Test log

Everything measured, kept or rejected, and what is left to try. **Look here before you start
anything**: a test already done (even a failed one) saves hours. **After a test, add a row**, even if
the result is negative.

## 2026-11-27: new base

Measured on the reference machine (RTX 3050 8 GB, i5-12400F, 32 GB), boat mission, **every build
prepared like a player does**: its own cache folder, shaders translated, one run that records the
pipelines (`VICTORY_PIPELINE_BOOK=1`), `victory warm --used`, then two measured runs.

| Build | 1st measured run | 2nd run | Picture |
|---|---|---|---|
| Our old base (fxceless + our fixes, PR #18) | 213-322 ms | — | good faces, white water in cutscenes |
| WASDeKeys/victory-low-end-pc (cool_fox) | 163 ms | **108 ms** | flat sky and backgrounds, black water with white spots |
| **Maykzera/victory-rtx3060 (trustno1 lineage)** | 103 ms | **98 ms (10.2 fps, p90 132)** | **realistic water in cutscenes and in game, detailed faces** |

Our earlier measurements of the other forks (trustno1 at "5 s a frame", avatarmanz at 377 ms) were
wrong: pipelines not built, `VICTORY_PIPELINE_WAIT_MS=10000` (12 s freezes) and a cache folder shared
between builds. Their architecture (list translation threads, submissions prepared together, reads done
on the card) prepares a frame in 34-62 ms against 100-130 ms for our old base: hence the new base. Our
old branch stays in the history.

Rest of the night (same method, new base):

| Test | Result | Decision |
|---|---|---|
| Without the XCL module (our emulation of the AMD instructions) | 104 / 103 ms against 103 / 98 ms with it | kept (PR #19) |
| Mesh draws launched by the card (kelvaroq 0041, ported) | 103 / 94 ms (p90 130); the stage only serves calls without a count buffer (~1 per frame here) | kept; to extend to calls with a count buffer |
| Mouse: mouse device (trustno1 #9, superboynick) + click = capture, Alt/Esc = release, X = walk | 98 ms, full run; a 1st version that re-centred the cursor without focus stopped the game around 285 s | kept |
| DLSS (Streamline) fixed: the game's FSR4 gets 1280x720 as its maximum size, DLSS renders below it; the evaluation goes into the `VICTORY_THIN` stream | performance: 102 ms, clean picture; quality (720p): 176 ms, the best-looking picture | kept (option), PR to come on base #21 |
| `VICTORY_WINDOWS_BAR_MB=3072` on 32 GB | 100 ms, peak RAM unchanged (20.3 GB), VRAM +550 MB | automatic under 24 GB only |
| Boat driven in game (7 min, `VICTORY_KEYS` held) | 8.6-9.9 fps on the water; W does not accelerate with this layout, A turns | to follow up |
| **Merge of Proudnoob4/VIctory-NVIDIA `nvidia-hybrid-v2` (PR #21)** | 105 / **93 ms (10.8 fps, p90 122)**; submitting thread 52-63 ms instead of 92-116 | kept; a few squares in the sky at times |
| `VICTORY_THIN_THREADS=4` instead of 5 on 6 cores (Maykzera, 26 Nov) | **87 ms (11.5 fps), p90 112, worst 178** against 93 / 122 | kept (launcher) |
| Heat haze 731a39171b568298 left out (foxhound `VICTORY_LEAVE_OUT`; it smears blocks, on the water too) | **84 ms (11.8 fps)**, p90 115 against 93 / 122; sharp faces, realistic water, in game reached | kept (launcher) |
| DLSS ultra performance (base #20) | 90 ms (11.2 fps), p90 114; clean picture, realistic water | to offer as a fast setting |
| Wave-wide lists by default (Felix2180, Xeltax; `VICTORY_WAVE_WIDE_ONLY`, + 926c…) | 115 ms / 108 ms; **dark rectangles on the water, floating dark blocks, squares in the sky** | rejected (picture) |
| foxhound base (trustno1-coder/VIctory-NVIDIA 9d6300f, readmet3xt-lab/foxhound), its settings, Series S 720p, same protocol | game upscaler: 103 / 95 ms; DLSS performance: 84 ms (13.5 fps in game); peak RAM 18.5-18.9 GB | flat turquoise water, detailed islands; **floating grey block on the water** with DLSS: we keep our base (more accurate at equal speed) |
| 43 "RT work only" passes skipped (Dathinel/victory-4gb #4, `rt_work_off_hashes.txt`) | 103 ms against 92 for the control run of the same moment; coloured dots on the bottom edge | rejected on our base |
| GPU timing per pipeline (`VICTORY_GPU_TIMING=1 VICTORY_D3D12_DRED=1`, diagnostics build) | ~70 ms of card per frame: 45 ms in ~3,400 indirect executions (mesh, ~5 groups each, ~13 µs apiece), 20 ms in ~1,180 dispatches; 1,100 pipelines, none above 4.3 ms (compute 776d835da5ffa646 4.3 ms, 14264103296aa43a 2.7 ms); ~700 global UAV barriers per frame | the cost is **per command**, not per pixel: lowering the resolution helps little (DLSS ultra 90 ms, 540p 84 ms). Lead: fewer card flushes (per-resource barriers), group the indirect executions |
| Proudnoob4 Pass 46-49 (17161c7: mimalloc, legacy LDS by default "trees", indoor haze, windows, pacing) merged separately (local branch `merge/proudnoob4-pass49`), same protocol | 100 / 94 ms; with its picture settings 97 ms | **rejected**: black characters, faces as texture mush, white water in cutscenes and in game, empty minimap. To retry commit by commit if Proudnoob4 fixes it |
| What limits us now | the card (~70 ms busy per frame), above all the fixed cost per command (~4,600 commands per frame) | per-resource barriers instead of global ones, group the indirect executions; get other machines measured (#10, #14) |

## How we measure

- Reference machine: **RTX 3050 8 GB, i5-12400F, 32 GB of RAM, Windows 11**, `--console series-s`,
  DLSS performance mode (`VICTORY_DLSS_MODE=performance`), settings of `series-s/launch_series_s.bat`.
- Same place every time: the **boat mission** (chapter I), from a **copy of the save restored before
  each run** (the game saves on its own: without that, every run starts somewhere else).
- `scripts/bench/run.sh`: 5 minutes, median and p90 of the "world" frames (300 draws or more), RAM,
  VRAM, three screenshots. **One run = ±100 ms of noise**: alternate at least A1 B1 A2 B2.
- What we read in the game log: the line `... fps, ... ms/frame (X preparing ...; Y on the GPU;
  Z copying back ...)` and the line `of the waiting for the card in settles, per frame, by what settled`.

## ⚠️ NVIDIA driver shader cache: set it before measuring

The NVIDIA driver keeps the pipelines it compiled in `%LOCALAPPDATA%\NVIDIA\DXCache`, **4 GB per
application by default**. VIctory compiles thousands of them: the cache is full after a few days of
tests, and **every exe compiled after that recompiles its pipelines at every launch** (300-500 ms a
frame instead of ~210, for identical code). NVIDIA Control Panel → Manage 3D settings → **Shader Cache
Size → Unlimited** (or 100 GB). A new (or moved) exe also needs 2-3 launches to fill its cache: compare
**warmed** builds, never a new exe against an old one.

Consequence here: the cache had been full since 26 Nov at 05:25. The tests marked **(to re-measure)**
below compared an exe compiled after that time with the old cached exe: their verdict is worthless until
they are redone. The tests of **variables** (same exe) remain valid.

**Two more traps (26 Nov, evening)**:
- **Translator changed = everything is cold.** The shader cache key contains the translator and the
  lists `VICTORY_WAVE_WIDE_ONLY`, `FIRSTLANE_OWN_ONLY`, `HEAP_USAGE`, `NAN_SCRUB`: changing any of them
  retranslates everything, and the ~3,500 start-up pipelines get new code that the driver recompiles
  (~400 per launch). Cold runs do not reach the boat in time: the keys land at the wrong moment and the
  game ends up elsewhere (beach, on foot). To do before measuring:
  `victory translate <shaders> --warm <list of hashes seen in game>` (same environment as the run, plus
  `VICTORY_HOST_WAVE=32 VICTORY_HOST_ROVS=1 VICTORY_VIEW_LIMIT=999999`; ~11-14 min), then 2-3 warm-up
  runs. Group your translator changes.
- **One cache folder for all builds**: the start-up pipeline list (`pipelines/early-pipelines.txt`) and
  the library are rewritten by every exe. A build of another fork launched once makes ours cold: give it
  its own `VICTORY_CACHE_DIR`.

## Where we stand (2026-11-26)

| | ms per frame (median) | fps |
|---|---|---|
| `feat/render` base (25 Nov) | ~474 | 2.1 |
| + mapping batch (#4) | ~394 | 2.5 |
| + targets/pipelines not redefined (#6, avatarmanz) + `UNWATCHED_FRAMES=2` | ~250-280 | 3.6-4.0 |
| + `MERGE_GAP=16` + `CARD_BUFFERS_MB=1536` (26 Nov) | **~206-248** | **4.0-4.9** |
| + picture update, mesh draws by the card, ports of 26 Nov evening (PR #18, warmed build) | 212-322 (heavy frames in steady state: 178-260) | 3.1-4.7 |

Where a frame goes today (~210 ms): **CPU preparation ~100-125 ms**, GPU 30-100 ms, copy back into
title memory ~60 ms, plus ~40-50 ms waiting for the card for a buffer copy ("title memory read"). In
parallel, the pass over the descriptor heap's buffer slots takes 60-150 ms on another thread (neither a
finer lock nor more threads made it faster: to profile).

### What the profile says (26 Nov, diagnostics build, `VICTORY_PROFILE=1`)

- **The CPU is not full**: ~2.4 threads busy out of 12. A frame is limited by **waits in series** and
  locks, not by computation.
- The thread that presents the frame waits **~37 ms per frame** on the `RELEASING` lock (`queue.rs
  release_held_now`), held by the submitting thread; that one itself waits **~15 ms** there for the
  flights' copy-back to finish (`host.rs relay_wait`, the "settles").
- `HostHeap::mirror` creates driver resources (`CreateCommittedResource`, ~5 ms) **while holding the
  heap lock**: the others wait ~10 ms. A finer lock is useless as long as creation happens under it.
- `texback::Target::new` waits ~10 ms on the `MAPPING` lock during page mappings
  (`copy_page_mappings`, ~11 ms of mapping/unmapping per frame).
- The submitting thread hashes and copies back textures (`blake3` ~5 ms, kept copies ~4 ms,
  `Staging::refresh` ~4 ms), possibly unchanged ones.

## Kept

| Test | Origin | Result | Where |
|---|---|---|---|
| Page mapping batch under a single lock | us | 474 → 394 ms (-17 %) | `xmem.rs map_aliases` (#4) |
| Render targets and pipelines not redefined | avatarmanz ca1d519f, ab86ef7f | -12 % | #6 |
| `VICTORY_UNWATCHED_FRAMES=2` | us | -17 % | launchers |
| Turquoise water: `VICTORY_WAVE_DEEP_SKIPS_NOT=<4 programs>` | us (analysis of the water programs) | correct water, same speed | launchers |
| `VICTORY_MERGE_GAP=16` + `VICTORY_MERGE_RUNS=65536` | existing variable | 233 / 216 ms against ~280; "title memory changed under shader writes" wait 31 → 0 ms; picture checked | launchers |
| `VICTORY_CARD_BUFFERS_MB=1536` (instead of 512) | trustno1-coder/VIctory-NVIDIA | 231 / 236 ms against ~280, same VRAM, lower p90 | launchers |
| DLSS performance mode | us | 351 → 292 ms, sharper picture | `VICTORY_DLSS_MODE` |
| Mouse: click = capture, Alt/Esc = release (no longer toggles) | cold-project-vi | comfort | `gameinput.rs` |
| `VICTORY_SKIP_PIPELINES` usable outside diagnostics builds | us | test tool | `pipeline.rs` |
| Textures picked by index on a separate thread (`VICTORY_HEAP_WORKER=1`) + PR #1220 (`HEAP_SYNC_ZERO_RUNS`), #1230 (`READBACK_COALESCE`), #1229, #1232 | fxceless d4a528ba and PRs by nicolasveilleux, spk-c | heap pass 99 ms instead of 130-180; frame 284-289 ms, never slower (warmed exe) | launchers |
| Faces and mouths that move: 16-bit values passed as is through typed views | fxceless PR #927 (tippythetaidum) | always on (the translation changes: shaders are retranslated once) | `victory-shader` |
| Black sun, banded sky: NaN or infinite colours set to 0 (`VICTORY_NAN_SCRUB=1`) | Humb8l (cool_fox-dlss), after trustno1-coder | in the shader cache key | launchers |
| Flickering ground: a 0-byte constant view stays empty | avatarmanz / fxceless 619b2dc5 | — | `descriptor/host.rs` |
| Texture written by the card copied back even if the list had already sent it | fxceless 19a45d19 (PR #1241) | — | `texture.rs` |
| Mesh draws launched by the card (`VICTORY_MESH_LAUNCH`, on): an amplification stage reads and bounds each record, no more pass or wait between two draws | stellarr-studio/kelvaroq patch 0041 (198 → 120 ms for them on an RTX 5080) | here ~295 ms, as before (the bottleneck is the CPU); `launch_code.rs` recompiled here from `launch.hlsl` | `launch.rs` |
| Copies back into title memory of textures written by the card, on a separate thread (`VICTORY_TEXBACK_ASYNC`, on) | Indy2l (trustno1-coder/VIctory-NVIDIA 2d1dc382, RTX 3060 + i5-12400F) | removes the copy-back thread's wait on the mapping lock | `texback.rs` |
| The game's DirectStorage buffer raised to 256 MB minimum (`VICTORY_DSTORAGE_STAGING_MB`) | CGBJ (cesarzxk) after cool_fox | — | `dstorage.rs` |
| "Legacy" MAD/MAC fused, denormal sources flushed to 0 as on RDNA 2 | Marvisak #1554 (Calvix2k), fxceless PR #1026 | black cores in the brightest reflections on the water; unit test on the card OK | `lower.rs` |
| Card choice (`VICTORY_ADAPTER`); unset: the most powerful one (Optimus laptops: no more Intel card) | trustno1-coder 033be27f + us | #14 | `host.rs` |
| Title memory windows in the card memory seen by the CPU (`VICTORY_WINDOWS_BAR_MB`, off; 3072 for 16 GB PCs) | "local" bd68f5e1 (avatarmanz, trustno1) | on 16 GB: no more 0x8007000E crash and 10-12 fps instead of 1.6-3 (RTX 3060, trustno1 #6); to measure here (32 GB) | `shadow.rs` |
| Published pipeline list, without game code (`victory pipelines export/import/merge`, `series-s/pipelines/series-s-rtx.txt`: 2,874 pipelines from the maintainer's cache past the boat mission, 444 KB of text, 234 states, 2,944 shaders by hash): built before the first session | us | fresh cache, outside the game: `victory shaders` 18 min (6 threads, 25,780 shaders); import + 2,944 containers found in the player's files by hash 7 min 12 s; `warm --used` 2,874 done in 6 min 19 s (6 processes, median 184 ms, 5 without a host pipeline, 0 skipped); translation hash identical to the maintainer's cache (d8d859e1b5d7b418). Final build (27/11 evening): fresh cache, import + `shaders` 24 min, `warm --used` 2,874 in 5 min, then a 15-minute session from a new game through the prologue to the boat mission: 92 ms median (10.9 fps). The list was then extended to 3,535 pipelines (that session + a community player's list) | `known/list.rs`, `pipelines.rs`, launcher [2]/[3] |

## Rejected (measured, don't redo without a new idea)

| Test | Origin | Result |
|---|---|---|
| `VICTORY_MERGE_RUNS=65536` alone | — | 290 / 343 ms: the wait moves to "title memory read" |
| "Masked" pass over changed bytes (`VICTORY_MASKED_MIN=64`, code present, off) **(to re-measure)** | cold-project-vi aff5fb2 (ported) | 388 / 280 ms, +1.3 GB of VRAM; the wait moves to "title memory read" (75 ms) |
| Heap pass in slices (`VICTORY_HEAP_ROLL=1`, code present, off) **(to re-measure)** | cold-project-vi 1141d35 | pass 110-170 ms instead of 63-150; 250 / 453 ms |
| Skipping the ray tracing shaders (`rt_off_hashes.txt` + `VICTORY_BVH_HOLD=1`) (speed to re-measure; the damaged water still holds) | trustno1-coder/VIctory-NVIDIA | 362 / 241 ms; grey and green water with black spots |
| `VICTORY_BVH_HOLD=1` alone | existing variable | 252 / 217 ms; dark green water and black spots |
| `VICTORY_HEAP_SYNC_TABLES=0` | existing variable | 265 / 276 ms, no gain |
| `VICTORY_ASYNC_PRESENT=1` | existing variable | 272 / 294 against 241 / 269 ms |
| TABLE_BUFFERS / SYNC_NULL_SKIP, DYNAMIC_SLOTS | uiliannn/VItoriaBr | slower here |
| Lookup cache (#1237) | fxceless | slower here |
| Heap pass on threads, memoisation, key cache | us | slower (shared lock) |
| Heap under 512 locks + lock-free fast path (branch `perf/heap-locks`) **(to re-measure)** | after avatarmanz 6c17ce9 | heap pass 144-155 ms instead of 97; 440 / 263 ms against 213: the lock is not the bottleneck |
| Wave64 "deep skips" shapes cut everywhere | — | worse; only the 4 water programs |
| DLSS ultra performance | — | only ~4 %, blurry picture |
| Whole builds of other forks on this machine **(to re-measure: new exes)** | Neptune, avatarmanz + cool_fox settings, cold-project-vi | 290-450 ms, worse picture or crash |
| Held reflections (Belobug e633374b) | Belobug | water still white |
| BVH builder alone | — | crash at 115 s |
| trustno1-coder/VIctory-NVIDIA as is, its settings (540p, game upscaler, RT off), warmed cache | trustno1 | ~5 s a frame on the 2nd run, 20 GB of RAM: unusable here |
| Full kelvaroq (fxceless f85101b + patches 0000-0064), its default settings, without DLSS | stellarr-studio | heavy frames ~300 ms: no better; we took 0041 |
| fxceless "run 279" settings we had without enabling them: `INDIRECT_WINDOW=1 CARD_REHASH_RUNS=1 BACK_OWN_VIEW=1 STAGING_POOL=0 CARD_WINDOWS_MB=1024` | fxceless | 295 ms, same as without |
| Water: `VICTORY_WAVE_WIDE_ONLY` without the two volumetrics programs (5dc3…, 6f22…) | cool_fox PR #24, fxceless PR #695 | the white water of the cutscenes stays white: it is not the cause. In game the water stays good and the run was faster (212 ms, a single run): to confirm |
| White water in cutscenes: `VICTORY_WAVE_DEEP_SKIPS=0` everywhere | — | white screen, black faces, black rectangles: worse |
| White water in cutscenes: without the 1,000 RT resolve shaders (trustno1's `rt_off_hashes.txt`) | trustno1 | water still white in cutscenes; correct in game, 242 ms |
| White water in cutscenes: `VICTORY_WAVE_WIDE_ONLY=none` | — | the whole picture flat (colour bands): one of the 4 default programs is needed; water still whitish |

## To try (by order of interest)

Take a row and open an *issue* "[in progress] …" so nobody does the same thing
(see [CONTRIBUTING.md](../CONTRIBUTING.md)). Size: S = a few hours, M = a day, L = more.

| # | What | Why | Leads | Size |
|---|---|---|---|---|
| 1 | Heap pass: understand where its 100-150 ms go | neither the lock nor threads helped; profile first | diagnostics build + `VICTORY_PROFILE=1`; avatarmanz 44629de (`shows_none`: 32,000 empty slots out of 41,000 skipped) | M |
| 3 | **"title memory read" wait** (~40-50 ms per frame) | a buffer copy into an area that shaders in flight write waits for the card | `host.rs blocked_by`: put the copy in the flights' queue instead of waiting | M-L |
| 3b | Create resources outside the heap lock | ~10 ms of waiting + 5 ms of creation | `HostHeap::mirror`: reserve the key, create outside the lock, publish; reuse texture copies | M |
| 3c | `texback::Target::new` without waiting for `MAPPING` | ~10 ms on the recording threads | read without the lock if possible, or release the lock between `copy_page_mappings` batches | S-M |
| 3d | Don't hash/copy back textures that were not written | ~5-10 ms on the submitting thread | check write tracking before `blake3` | S-M |
| 4 | Copy back into the game (~60 ms) | on the critical path | profile (`VICTORY_PROFILE=1`, PDB next to the exe); comparisons on the card | M |
| 5 | CPU preparation (~100 ms) | the biggest item | profile; `TablePlan` (codec_d e05d463) | L |
| 6 | Black faces at times | picture | PR #927 ported (moving faces); still to see whether any remain: avatarmanz e4245f3c ("black-face texture fixes", big commit tied to others) | M |
| 7 | ~~Black sun, banded sky~~ | done (`VICTORY_NAN_SCRUB=1`) | | |
| 8 | Stale buffers with `UNWATCHED_FRAMES=2` | picture | codec_d 18bdf79: depends on `pool.rs`/`residency.rs` that we don't have (6 conflicts in `draw.rs`); port only the `draw.rs` part | M |
| 9 | **White water in cutscenes**, white shallow water, ghost rectangles | picture | the 4 water programs do run in cutscenes (bound around 220 s); neither volumetrics, nor deep skips, nor RT, nor wave-wide lists change anything (see Rejected). Next: capture a cutscene frame (`VICTORY_CAPTURE_AT`, diagnostics build) and follow who writes the water's HDR target; `docs/deserialize-pipelines.md` | M-L |
| 10 | Variables to measure | no code | `VICTORY_WIDE_REFRESH=1` (1 run: 221 ms), `VICTORY_TEXTURE_STAGE_MS=24 VICTORY_WINDOW_KB=1024 VICTORY_WINDOW_TAILS=1` (1 run: 217 ms, p90 383 ms instead of ~500: to confirm), `VICTORY_JOIN_COPIES=1`, `VICTORY_INDIRECT_WINDOW=1`, `VICTORY_CARD_REHASH_RUNS=1` | S |
| 11 | Card memory written directly (Resizable BAR) | -2 GB of shared memory | cold-project-vi 0030cd4 | M-L |
| 12 | **Other machines** | we only know the RTX 3050 | measurements on AMD, 12 GB, other CPUs: very useful | S |
| 12b | **Validate the automatic profiles** (`materiel` in `series-s/reglages.json`) | the steps off the reference (6 GB: 768 MB buffers and `TEXTURE_MAX=2048`; 11 GB and more: 2048 MB; 8 threads: 3/2/2/3; 16 threads: 6/4/4/6; 16 GB of RAM: one step lower and 1024 MB) are reasoned choices, **not measured yet**; only the reference PC is (settings unchanged, checked by `scripts/check-profils.ps1`) | "Give feedback" reports from the launcher (`feedback` form); adjusting a step = one line in `reglages.json` | S |
| 13 | edupode/cold-project-vi ports (off by default in code; launcher defaults below once measured) | measured on the RTX 3050 (clean runs, 27/11): see Kept | `VICTORY_WAVE_WIDE_DIGESTS`, `VICTORY_VRAM_RESERVE`, `VICTORY_TEXTURE_MAX` (less sharp, for 6 GB), `VICTORY_POOL_FULLEST`; write watch searches its ranges instead of scanning them (no switch, same answers) | S |
| 14 | **Frame generation** (DLSS-G through Streamline, ported from trustno1-coder 6395727/22c6be2 and pepezin-ai 1469efe; `VICTORY_FRAMEGEN=1`, `VICTORY_FRAMEGEN_X=2..4`, off by default) | 2x the frames shown for the same cost per rendered frame, if the card generates them | Inputs from every upscale: the FidelityFX dispatch (DLSS, `VICTORY_FFX_DIR`) and the title's own upscaler (its dispatch method's inputs, our default). Checked without the title on the RTX 3050 (`cargo test -p victory-gpu --lib streamline_answers -- --nocapture` with `VICTORY_FRAMEGEN=1 VICTORY_FRAMEGEN_DIR=<folder>`): Streamline loads, Reflex and PCL supported, **DLSS-G: `eErrorNoSupportedAdapterFound`** ("not supported on current hardware", as expected on RTX 30), so plain present. To do: the run with dlssg-to-fsr3's `nvngx_dlssg.dll`; frames shown, picture of generated frames, jitter sign (`VICTORY_FRAMEGEN_OWN_JITTER`), cost per frame | M |

### Ported picture fixes (to verify on the prologue)

Ported 2026-11-27 from other forks (see `git log`), **not built or run yet**. Rows marked T change the shader
translator: one retranslation (`victory translate ... --warm`) covers them all. Check each with a before/after
screenshot, then move it to Kept or Rejected.

| What | Commits (source) | Where to look | T |
|---|---|---|---|
| Rainbow arms and faces: `v_nop`/`v_pipeflush`/`v_clrexcp` take no literal; A4B4G4R4 (191) is 2 bytes a texel | 54d736c0, b1a67794 (avatarmanz PR #431, LeonardoS11, from ZigZagBoi1/ScarlettPort) | opening cell and corridor, several runs (it was intermittent) | T |
| Level query on array textures takes no slice: three alpha-clip programs validate again | d1a4e229 (avatarmanz, after upstream 13091f6e) | alpha-tested surfaces (foliage, fences, decals) present; fewer refusals in the `victory shaders` log | T |
| Compute `ds_swizzle` from an inactive lane reads 0; unused image-query extents read 0 | f3023908, 122e23f9 (Dathinel PR #7, #8) | no visible change expected; watch for regressions | T |
| Comparison sampler keeps its min/max reduction; heap anisotropic samplers keep point mips | 4e4ae2d0 (Vice-Tay, rayane-creato1r), 48f59c1a (Flare, upstream #2031) | faces and hair sharpness (oz saw a blur once with the point-mip half) | T (first) |
| SDWA WORD_0 sign extension | a4a8e0e6 (write_half part of fx 58de9bf4c, upstream #3124) | 16-bit packed data; no visible change expected | T |
| POPS collision id seeded with wave id 1: no spin in the ordered-section wait | 7247fbec (after Marvisak 7ac1f54d8, VastyViz #2395) | skin (`PS_PedTex*`), hair; frame time of close-ups | T |
| Transmission tile allocation on wave32 (container f5d10cde..., our shader 1aa247ed3653b616, not skipped) | dff5bba2 (trustno1 PR #45, xss3009) | dark rectangles on the water: bridge approach, open sea, first boat | T |
| A big compare's first slice is compared | 5aa3de06 (after fx 81d54b06f, PiloteCode #1407) | stale bytes after big copies; no visible change expected | |
| NaN scrub skips image stores through integer texture UAVs | 1cafcbfb (ours; idea from Vice-Tay 6f179dd10) | with `VICTORY_NAN_SCRUB=1`: nothing should change but integer images (IDs, masks) | T |
