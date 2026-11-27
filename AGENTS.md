# AGENTS.md: guide for AI assistants working on Sagouindupm | VIctory

Read this first. It saves you hours of exploration and saves your user's tokens.

## What the project is

A fork of [VIctory](https://github.com/Marvisak/VIctory): a compatibility layer (Rust, Windows,
Direct3D 12) that runs an Xbox Series title from the user's own files. It loads the title's PE,
provides the Xbox OS services (HLE), translates the console's GPU commands and AMD GCN/RDNA shaders
to D3D12 and DXIL, and keeps the title's unified memory and the PC's separate CPU/GPU memories
in sync.

**This fork's single goal:** the best frame rate and a picture faithful to the Series S on a
**low-end PC in the Series S profile** (`--console series-s`): 8 GB card, mid-range 6-core CPU,
16-32 GB RAM. The reference machine is an RTX 3050 8 GB + i5-12400F. Since 27/11/2026 the tree is
based on the trustno1-coder/VIctory-NVIDIA lineage (avatarmanz architecture, Maykzera's RTX 3060
tuning): ~10 fps in the boat cutscene and on the boat in game (98-103 ms a frame, shaders and
pipelines prepared). Targets: 10-12 fps in cutscenes, 22 fps in game, picture 1:1 with the Series S.

**Before measuring anything, prepare like a player does** (or your numbers are meaningless):
`victory shaders <shaders>` with `VICTORY_HOST_WAVE=32 VICTORY_VIEW_LIMIT=999999` (NVIDIA), one
run with `VICTORY_PIPELINE_BOOK=1`, then `victory warm --used`, then measure the next runs. Give a
build of another fork its own `VICTORY_CACHE_DIR`: builds share the pipeline library and its
start-up list otherwise. Never `VICTORY_PIPELINE_WAIT_MS=10000` (12 s freezes); 150 is right.

## Hard rules

- **Never add game files**, dumps, raw logs or decompiled title code to the repository. Never patch
  the title's executable on disk.
- **GPL-3.0-or-later only.** No GPL-2.0-only code (XCL, the original Mandy) or incompatible licences.
  The upstream lineage carried an XCL-derived `crates/victory-loader/src/console_insn*.rs`: it is
  removed here and AMD-only instructions are emulated by our own code in `crash.rs` and
  `code_room.rs`. Never bring it back.
- **Keep authorship.** When porting another fork's work, `git cherry-pick -x` or write
  "after <author> <commit>" in the message. Never present someone else's work as new.
- **Measure, don't guess.** A speed change needs a median of at least two alternating runs from the
  same save (`scripts/bench/run.sh`); one run is ±100 ms of noise. A picture change needs a
  before/after screenshot.
- **New behaviour goes behind a `VICTORY_…` environment switch**, documented in a comment where it
  is read, **off by default** until measured. That is the house convention: every switch is
  described where it is read (`grep -rn "VICTORY_NAME" crates`).
- **Record every result in [docs/essais.md](docs/essais.md)**, negative ones included, in the same
  PR. Check it before proposing an idea: many obvious ideas were already measured and rejected.
- **Coordinate:** before starting, check the open issues for `[en cours]` / `[in progress]` claims on
  the same task or code area (see [CONTRIBUTING.md](CONTRIBUTING.md)). Don't edit an area someone
  else has claimed; suggest your user comments on their issue instead.
- Say "AI-assisted" in the PR description.

## Map of the code

| Crate / file | What it does |
|---|---|
| `apps/victory` | the executable: `victory run <game exe> --console series-s --execute` |
| `crates/victory-loader`, `victory-pe` | loading the title's PE, in-memory hooks |
| `crates/victory-hle` | Xbox OS services: memory (`modules/xmem.rs`), input (`modules/gameinput.rs`, `controls.rs`), saves, audio |
| `crates/victory-shader` | GCN/RDNA → DXIL translation (`src/emit/`: `lower.rs`, `wave.rs` for wave64 on 32-lane NVIDIA cards) |
| `crates/victory-gpu/src/host.rs` (~10,500 lines) | submissions, "flights" sent ahead (`late`), settles (waits for the card), the relay thread that copies results back into title memory, the card-side copies' compare (`CARD_COMPARE`) |
| `crates/victory-gpu/src/draw.rs` | per-draw state, the pass over heap buffer slots (`heap_buffers`) |
| `crates/victory-gpu/src/descriptor/host.rs` | host descriptor heaps mirroring the title's (`HostHeap::mirror`, striped locks `Made`) |
| `crates/victory-gpu/src/shadow.rs` | host copies of title memory (CPU side, card side), dirty ranges, merges under shader writes |
| `crates/victory-gpu/src/queue.rs`, `queue2.rs` | presents, Series S two-plane compose, the copy queue |
| `crates/victory-gpu/src/texture.rs`, `resource.rs` | textures, uploads, format stand-ins |
| `crates/victory-gpu/src/list/thin.rs`, `host/card_steps.rs`, `land.rs` | the avatarmanz architecture: list calls translated by our threads (`VICTORY_THIN`), reads of title memory under flights done on the card (`VICTORY_CARD_STEPS`), landing on helper threads |
| `crates/victory-gpu/src/pipeline/known.rs`, `apps/victory/src/warm.rs` | the pipeline book (`VICTORY_PIPELINE_BOOK=1`) and `victory warm` |
| `series-s/` | the launchers and every setting we ship: `reglages.ps1` (the one place for the Series S settings), `Sagouindupm.ps1` (console), `Sagouindupm-Launcher.ps1` (graphical, for novices) |
| `scripts/bench/` | the measuring scripts |

Big files: don't read `host.rs`, `shadow.rs` or `draw.rs` whole. `grep -n` for the switch or the
function, then read around it.

## How to read a run

The log prints, about once a second:

```
3.9 fps, 256.9 ms/frame (101.9 preparing: copies 32.7 ms 96.5 MB, textures 10.9 ms ...; 96.6 on the GPU;
59.2 copying back 28.3 MB), 5008 draws/frame, ... heap buffers 1.0 passes 143.7 ms ...
```

- `preparing`: CPU time to turn the title's command lists into D3D12 work.
- `on the GPU`: the card's time. `copying back`: the relay landing results into title memory.
- `heap buffers … passes … ms`: the buffer-slot pass, on another thread.
- `of the waiting for the card in settles, per frame, by what settled: …` says why the submitting
  thread had to wait for the card (e.g. "title memory changed under shader writes", "title memory
  read").
- World frames are those with 300+ draws; menus and loading run at 30 fps and mean nothing.

## Where help is needed

The ranked list is **"À essayer" in [docs/essais.md](docs/essais.md)**. In short:
1. CPU preparation (~100 ms) and the lock shared by the heap pass and the title's recording threads.
2. The ~40-50 ms a frame the submitting thread waits for a buffer copy into memory that shaders in
   flight write ("title memory read" settles).
3. The ~60 ms copy-back.
4. Picture: black faces at times, black sun / banded sky (NaN), white shallow water, ghost
   rectangles on water.
5. Measurements on other hardware (AMD cards, 12 GB cards, other CPUs).

Other forks worth mining (all GPL-3.0; credit them): avatarmanz/VIctory-7900XTX, cool_fox mirrors
(e.g. DoubleGhosty/cool_fox), trustno1-coder/VIctory-NVIDIA, edupode/cold-project-vi,
uiliannn/VItoriaBr, fxceless/victory-fork. Their whole builds ran slower than ours on the reference
machine, but single ideas from them won (see "Gardé").

## Working efficiently

- Build: `cargo build --release -p victory`; checks: `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test -p <crate>`. A diagnostics build (`--features diagnostics`) adds captures
  (`VICTORY_CAPTURE`, `VICTORY_CAPTURE_AT`, `VICTORY_DUMP_TARGETS`) and is slower.
- **NVIDIA shader cache:** the driver keeps compiled pipelines in `%LOCALAPPDATA%\NVIDIA\DXCache`,
  4 GB per application by default. Once full, every newly built exe recompiles its pipelines at
  every launch and looks 1.5-2x slower for identical code. Ask your user to set NVIDIA Control
  Panel → Manage 3D settings → Shader Cache Size → Unlimited, and compare warmed builds only
  (2-3 launches each), never a new exe against an old one.
- Don't compile while a measurement runs on the same PC, and don't run two game instances at once:
  both ruin the numbers.
- Prefer environment switches you can A/B without rebuilding. Many already exist, off by default:
  grep for them before writing new code.
- Keep PRs small and on one topic; reviewers (and other agents) must be able to follow them.
