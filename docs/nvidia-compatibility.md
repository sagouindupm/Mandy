# "NVIDIA Compatibility" profile and black screen diagnosis (2026-11-25)

Machine: Windows 11, Ryzen 5 5600X, **RTX 3060 12 GB** (driver 621.40 / UMD 32.0.16.2140),
32 GB of RAM, 24 GB paging file, PowerShell 5.1.

The title's executable was **not run, read or modified** in this work. Everything below was
validated with our own programs and with the VIctory test suite.

## Why this fork and not VIctory-7900XTX

Both share the same base (7900XTX is a fork of this one). The difference is where each was measured:

| | VIctory-7900XTX | victory-fork (this one) |
|---|---|---|
| Tested on | RX 7900 XTX, 64 GB | **RTX 4070 12 GB, 32 GB** |
| Memory in game | 40–46 GB private ("64 GB recommended") | 23–27 GB |
| `play.ps1` settings | RDNA 3 | set measured on NVIDIA (`diagnostics/README.md`) |

With 32 GB of RAM, 7900XTX is already outside what its own author recommends.

## What the RTX 3060 accepts and refuses (measured)

`victory-gfxcheck` (our own program, `E:\GTA6.Xbox.v1.0.2.1\victory-gfxcheck`): **12 of 12 tests
passed.** Facts that matter for the layer:

| Fact | Measured value | Consequence in VIctory |
|---|---|---|
| Waves (wave lanes) | min 32, max 32 | The console's wave64 programs need the translator's 32-lane forms |
| Compute `[WaveSize(64)]` | **refused**, `E_INVALIDARG` (0x80070057) | A program translated that way loses its dispatches: the translator picks 32 lanes through `set_host_max_wave` |
| Shader-visible view heap | 1,000,000 accepted; 2^20, 2^21, 2^22 **refused** | No "raw twins"; `probe_view_limit` clamps indices to the null slot 999,999 |
| Shader model | 6.8 | Meets it (6.6 required) |
| Mesh shaders | tier 1 (DispatchMesh tested with read-back) | Meets it |
| Ray tracing | tier 1.2 | Available; off by default |
| Binding / heap tier | 3 / 2 | Meets it |
| 64-bit atomics on typed resources | yes | Meets it |
| VRAM | budget 11,342 MiB; 4 GiB allocated and released without a leak | Evict/MakeResident work |
| Synchronization | 2,000 CPU↔GPU round trips (1.5 µs); 200 direct→compute→copy chains without deadlock | No deadlock in the queues |
| Presentation | 300 frames with vsync, every `Present` = S_OK | Flip-discard swap chain works |

VIctory's own shader translation (`cargo test -p victory-shader --features host`):
**217 passed, 0 failed, 3 ignored**. The tests that execute translated DXIL ran on the RTX 3060
(e.g. `wave64_tests ... ran on the hardware device`). The "diff" tests that compare against a
console compiler were skipped: that compiler does not exist on this machine.

Conclusion: **the card, the driver, DirectX 12 and the translator work on this machine.** The
black screen does not come from a basic incompatibility of the RTX 3060.

## What is known about the run that stayed black

No log was written (the log only goes to the console). What was left in the cache
(`%LOCALAPPDATA%\victory\cache`) from the 21:29–21:39 run with 7900XTX:

- 25,718 shaders translated in the cache (full translation).
- The `amd_fidelityfx_upscaler_xbox` DLL loaded late (21:36): the title got past initialization.
- A **109 KB** pipeline library and 16 pipelines in the "early" list; 5 of them took
  **1.0–1.3 s each** for the driver to compile. A complete library has ~3,400 pipelines and 366 MB.

Reading: the title made few pipelines, the GPU sat at 3 %, and the NVIDIA driver had a cold
cache. The fork itself documents "black frames while pipelines are rebuilt" on a cold cache.
This is **consistent** with the black screen, but **not proven**: only a log from the next run
says which stage stopped. Hence the watchdog below.

## What was added

### 1. Black screen watchdog — `crates/victory-gpu/src/gfxwatch.rs` (new)

Records, once and with the seconds since device creation, each stage reached:

1. D3D12 device created · 2. first shader translated · 3. first host pipeline created ·
4. first title command list submitted · 5. first draw · 6. first dispatch ·
7. first title Present · 8. swap chain created on the window · 9. first frame shown
(Present S_OK) · 10. first frame that is not black

Every 5 s, if the window has gone 10 s without a frame, it writes a `BLACK WINDOW` line with: last
stage, stages not reached, title / host / black / failed Presents, last HRESULT, lists,
draws, dispatches, shaders ok/failed, pipelines ok/refused/still being made, VRAM, shared
memory, process private memory, and whether the device was removed — plus a verdict in
words (e.g. "the title has not presented yet while pipelines are being made: pipeline
compilation on the first run").

`VICTORY_GFX_WATCH=<seconds>` (default 5; `0` turns it off). It only reads atomic counters: no lock
on the title's threads.

Hooks (one line each): `host.rs` (device ready, swap chain created, Present result),
`queue.rs` (title Present, ExecuteCommandLists), `pipeline.rs` (shader translated or not),
`pipeline/make.rs` (pipeline created or refused), `lib.rs` (module).

Regression tests: `gfxwatch::tests` (3), all passing.

### 2. NVIDIA Compatibility profile — `scripts/play-nvidia.ps1` (new)

- Detects the GPU (Win32_VideoController); refuses to run without NVIDIA unless `-Force`.
- Uses the set of options measured on the RTX 4070 12 GB / 32 GB (`diagnostics/README.md`), all
  checked to exist in the code.
- `VICTORY_D3D12_DRED=faults` (cheap device-loss tracing) and the watchdog turned on.
- `-Console SeriesS` passes `--console series-s` (the title asks for less memory); default Series X,
  which is the tested one.
- Optional `-RayTracing` (`VICTORY_BVH_BUILDER=1`), off by default.
- `-Plain`: only the build defaults, to isolate problems.
- Warns if free commit memory is below 30 GB.
- **Writes the full log** to `%LOCALAPPDATA%\victory\logs\run-<date>.log` (outside the source
  tree, as `docs/asset-policy.md` requires), with the GPU, the driver and every option at the top; the
  window only shows stages, watchdog, warnings and errors; on exit, it prints the last stage lines.

What is NVIDIA-specific is already decided by the code from what the card reports (32 lanes,
1,000,000 heap, 64 KiB constant views, indirect dispatches of up to 65,535 groups); the
AMD profile (`VIctory-7900XTX\scripts\play.ps1`) was not changed.

### 3. Standalone graphics tests — `E:\GTA6.Xbox.v1.0.2.1\victory-gfxcheck` (new)

Our own crate (outside both VIctory trees). Tests: window; device, adapter and features;
solid colour through the swap chain with read-back; triangle; texture upload and sampling;
compute checking all 1,024 values and the lane count; mesh shader; pipeline creation
(32 distinct compute, graphics, mesh, and WaveSize(64)); CPU↔GPU and cross-queue synchronization with
timeout (detects deadlock); descriptor heaps at the sizes the layer asks for; 300 continuous
frames with timings; allocation of up to 4 GB of VRAM, residency and release. Report also in
`%LOCALAPPDATA%\victory\gfxcheck\`.

## Memory with 32 GB of RAM and 12 GB of VRAM

- Measured in this fork, on a machine with the same RAM/VRAM: 23–27 GB of PC memory and 9.2–9.6 GB of the card's 10.3 GB
  budget in game, with no budget overrun in any sample.
- Here: 11.3 GB VRAM budget, 19 GB of free RAM and 41 GB of free commit before running.
  The headroom is small: close browsers and other programs.
- The paging file prevents commit failures (error 1455), but does not replace RAM: whatever is
  paged out becomes slow. `-Console SeriesS` is the lever to make the title ask for less memory.

## First real runs (2026-11-25, 22:12 and 22:18)

Run 1 (7900XTX shader cache, cold driver):

- All 10 graphics stages reached within 5 s; menus at 30 fps; no errors, no device
  loss; 22.5 GB of RAM and 9.4 of 11.2 GB of VRAM.
- In the "Vice City, two years ago" scene: 800–950 pipelines queued and 90–920 draws dropped per
  frame ("without a host pipeline: being made"). **This is the black screen with only the subtitle**: the scenery
  draws wait for pipelines the driver is still compiling; the subtitle already had its own.
- `shaders from the cache ... hits=0`: the 7900XTX shader cache is of no use to this fork (different
  translator, different key). The run's 1,122 shaders were translated during play.
- It did not hang: the window was closed ("title did not finish suspending in time; ending the title").

Fixes:

- `victory translate E:\GTA6.Xbox.v1.0.2.1\shaders.rpf --warm all --jobs 12`: 25,636 shaders
  translated into this fork's cache in 4m51s, 253 do not translate (ray-traced geometry and
  shadows, already known). In run 2 the cache started hitting (hits=229 against 0).
- `VICTORY_PIPELINE_WORKERS` (new in this fork, `pipeline/make.rs`, with a test): the number of workers
  was fixed at half the threads (6 on a 5600X). `play-nvidia.ps1 -Warmup` uses threads − 2.

Run 2 (library of 825 pipelines from run 1):

- 653 pipelines loaded from the library instead of compiled; 1,656 started early, in the menus.
- Measured pace: ~570 pipelines/minute while they are light; then ~10/minute with the 6 workers and
  3 more driver threads (Idle priority) taking the whole CPU (100 %): the remaining ones take
  ~30 s each. There is no hang; it is compilation, and the 6-core CPU is the limit.
- My mistake, now understood: shader translation running at the same time (12 threads, normal
  priority) starved the workers (below normal) of CPU from 02:20 to 02:24.

Practical consequence: the first runs are warm-up runs. Each one adds pipelines to the
library (`%LOCALAPPDATA%\victory\cache\pipelines\host-pipelines.bin`), and the following ones
load them in milliseconds. A queue ("being made") at 0 is the sign that the picture can appear.

## Run 3: the world appears (22:42)

With shaders in the cache (2,672 hits out of 2,676) and the 234 MB pipeline library, the pipeline
queue reached 0 and the world rendered: ~6–7 fps, 150 ms per frame, 2,000–3,700 draws, no
errors. The remaining dropped draws (200–290 per frame) belong to the ray tracing structure
builder, which is off; that is why everything the sun does not light directly is black.

## Ambient light (ported from VIctory-7900XTX)

`crates/victory-gpu/src/ambient.rs` (from VIctory-7900XTX, idea by beloooo / VIctory-Belobug-AMD,
GPL-3.0-or-later): right after the title program that resolves the screen probes' diffuse light
(`CS_ScreenProbes_TemporalResolve_*`), it raises the light to an estimate taken from the frame itself (the light
that visible surfaces emit, averaged over 64x36 cells, as 1st-order harmonics on 160x90
points), with shading in creases derived from depth. It only acts while the ray tracing
structure builder is off.

- Hooks: `lib.rs` (module), `pipeline.rs` (shader role, from the name in the container),
  `draw.rs` (tables rebuilt per bind and the call after the dispatch, copied from 7900XTX).
- `VICTORY_AMBIENT`: a number (fraction of the estimate; `play-nvidia.ps1` uses 0.8, like
  7900XTX's `play.ps1`), `constant:<level>`, or `0` to turn it off. `-Ambient` in the script.
- If this fork's table layout is not what the light expects, it does nothing and the log says
  "no stand-in for the indirect light: other tables" (no risk to the picture).
- Tests: `ambient::tests` (3), including creating the passes on the RTX 3060: all pass.

`victory-gpu` suite with this: 201 passed, 1 failed (`watch::tests::written_pages_are_reported_to_every_asker`,
which passes 3 out of 3 when run alone: global write-watch state shared with tests running in
parallel; `watch.rs` was not touched).

## Optimization (run 4 onward)

Run 4 (ambient light on, everything compiled: shaders 2,699/0, pipeline queue 0): 0.6–4.4 fps
in a cutscene. The frame is the CPU copying the title's memory: texture updates ~219 ms, copies
180–210 MB, copy-back 55–110 ms, the GPU nearly idle. Compiling more changes nothing.

What went into `play-nvidia.ps1` (on by default; `-NoFast` turns it off):

| Option | What it cuts (the fork author's estimate, not measured in game) |
|---|---|
| `VICTORY_FILLS_PLAIN=1` | 8,000–12,000 small fills per frame without a kernel call (~3 ms) |
| `VICTORY_STEP_INDEX=1` | copy lookup by index instead of a lock (1.5–4 ms) |
| `VICTORY_LIGHT_REFRESH=1` | refreshes of untouched blocks without lock or kernel (copies 43→30 ms indoors, 80→57 outdoors) |
| `VICTORY_ABSENT_ONCE=1`, `VICTORY_ABSENT_MS=told` | does not ask Windows again about blocks already known to be absent (44 ms/frame outdoors) |
| `VICTORY_REFRESH_ONCE=1` | memory updated once per submission |

The `alt` modes of these options have independent 20 s clocks (each starts at first use),
so they do not alternate together: compare two runs (normal and `-NoFast`), not one.

`release` build (`cargo build --locked --release -p victory`, `-Build release` in the script): without the
`diagnostics` feature, which adds per-frame statistics. The watchdog and ambient light are in it too;
the detailed `framelog` lines and black frame detection stay in `diag` only.

### Measured (run of 2026-11-26 00:06 against the one of 2026-11-25 22:51)

Different scenes (the second with ~2x more draws), so the numbers are indicative:

| CPU per frame | Without the 5 options | With them |
|---|---|---|
| Texture updates (all threads) | 218.9 ms | 10.7 ms |
| List steps (copies, fills) | 255.1 ms | 55.6 ms |
| Write-watch and lookup under flights | 68.8 ms | 14.0 ms |
| Update flush | 52.5 ms | 8.4 ms |
| Recording, all threads | 2,175 ms | 469 ms |

In the same run, the first version of `-MoreTextures` (1 GiB windows, 768 MiB buffers on the
card) left 1,760 MB of copies on the card against 3,126 MB, and the GPU time on the title's lists
went to 235–287 ms per frame against 28–53 ms: buffers in PC RAM read over the bus. The CPU gain
vanished behind that. `-MoreTextures` now only raises the texture ceiling to 94 %.

### Run of 2026-11-26 00:14 (`-MoreTextures` fixed) and 00:21 (`-Display 1080p`)

- 00:14: world with 5,000–6,800 draws at 6.4–6.9 fps (145–156 ms); in the 22:42 run, 2,000–3,700
  draws gave the same speed. GPU almost always busy (idle 7–18 ms): title lists
  74–96 ms, layer lists (hashing and comparing the copies on the card) 36–46 ms. New bottleneck: GPU.
- 00:21: `VICTORY_DISPLAY_MODE=1920x1080` was reported to the title (log: "display mode reported to
  the title width=1920 height=1080") and it **kept rendering at 2560x1440**. No gain.
- 00:21: 7.4–7.7 fps with 4,000–6,000 draws, and after ~4 min in the world the card hung:
  `0x887A0006` (DEVICE_HUNG), DRED "page fault" with no address, VRAM at 10.6 of 11.3 GB (texture
  ceiling at 94 %). Hypothesis, not proven: VRAM budget at the limit, paging, one command
  goes past 2 s (Windows' default TDR) and Windows resets the card. `-MoreTextures` is no longer
  recommended on 12 GB cards.

### The title's dynamic resolution and checks on the card (2026-11-26)

- **Why the title does not lower its resolution:** it measures its own GPU time with four
  timestamps per frame, in pairs. The layer writes them all when the frame's last submissions
  finish, microseconds apart, so each pair says "the GPU did nothing", and dynamic resolution
  stays at its maximum (2560x1440). `VICTORY_DISPLAY_MODE` does not change that.
- **Ported from VIctory-7900XTX:** `VICTORY_GPU_LOAD_MS=<ms>` (`queue.rs`, `gpu_load`/`as_loaded`):
  the start of each pair is written `ms` before the end, and the title "sees" the GPU take that long. Above
  the 33 ms budget it can lower the resolution on its own. Test
  `queue::tests::a_pair_of_timestamps_shows_the_load_it_is_given`. Script: `-GpuLoadMs 40`.
  The effect on the title is not measured (the 7900XTX author did not record it either).
- **Measuring:** `ambient.rs` now logs "the title's render size" on every change of the diffuse
  light's resolution.
- **`VICTORY_CARD_HASH_WIDE=1`** joined the default savings: the hashes of the copies on the card use 64
  threads per 64 KiB chunk instead of one (the fork's PR #1014, measured on an RTX 3060). Here the
  layer's hash and compare lists were 36–46 ms of GPU per frame with the GPU as the limit.
- `victory-gpu` suite: 202 passed, 1 failed (the WARP test already known to be flaky).

Discarded: `VICTORY_INPLACE` (title memory as a GPU heap) creates the heap in PC RAM, the
same slow path over the bus.

## About 60 fps

It is not reachable with this layer: the title locks itself to 30 fps (as on the console), and in the
open world this fork measures 7–12 fps on an RTX 4070 with an i7-14700KF (28 threads). The measured limit is
the thread that prepares submissions to the GPU (CPU), not the card: the GPU is ~3 % busy in the logs here.

## `victory-gpu` suite after the changes

`cargo test --locked --release -p victory-gpu`: **197 passed, 1 failed, 5 ignored.** The failure,
`indirect::tests::warp_budget_saturates_without_wraparound_and_preserves_draw_fields`, predates
this work: it runs on the software adapter (WARP), in `indirect.rs`, which was not touched. This
fork changed the assertion to require that records 1 and 2 be the ones kept, which depends on the order
in which threads do the budget's `InterlockedAdd`. The version of the same test in
VIctory-7900XTX, which does not have that requirement, passes. It is unrelated to the black screen.

## Limits (what was NOT validated)

- The black screen itself: the title was not run here. The cause is only confirmed by the log of the next
  run (`graphics stage reached` and `BLACK WINDOW` lines).
- The watchdog inside a real run of the title (unit-tested only).
- In-game performance and stability on the RTX 3060.
- The full `victory-shader` suite with comparison against the console compiler (absent).

## Commands

```powershell
cd E:\GTA6.Xbox.v1.0.2.1\victory-gfxcheck
.\target\release\victory-gfxcheck.exe
```

```powershell
cd E:\GTA6.Xbox.v1.0.2.1\victory-fork-main\victory-fork-main
cargo build --locked --profile diag -p victory --features diagnostics
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts\play-nvidia.ps1 -Exe E:\GTA6.Xbox.v1.0.2.1\game_scarlett_master.exe
```

## Frame pacing (1% low, P95/P99, hitches)

New module `crates/victory-gpu/src/pacing.rs`. Every successful host Present records the time since the previous one. Every 10 s the log has a `frame pacing, last 10 s` line with:

- the average fps, the 1% low and the 0.1% low;
- the median, the P95, the P99 and the longest frame time;
- how many frames went over 100 ms, 250 ms and 1 s, in the window and since the start.

The script already shows this line in the terminal. `VICTORY_PACING=0` turns it off. There are 3 tests (equal frames; a 2 s hitch; empty window).

## Measurements from the 11/26 07:49 run (-GpuLoadMs 40, CARD_HASH_WIDE=1)

| | before (00:14) | now |
|---|---|---|
| fps in the world | 6.4–7.7 | 7.7–9.1 |
| upload+hash and compare lists on the card | 36–46 ms | 11–14 ms |
| game lists on the card | 74–96 ms | 62–92 ms |
| submitting thread, working per frame | – | 86–101 ms |

- The resolution did not change with 40 ms: still 2560x1440. The next test is 100 ms.
- The card is busy ~84% of the frame. It and the submitting thread take ~90 ms each, so both limit together.
  - Lowering the resolution would relieve the card and also shrink the copy-back (50–70 MB per frame).
  - On the CPU side, the biggest item is descriptor heap mirroring: 19.6 ms per frame, of which 14.2 ms create the host slots for ~12,600 new records.

## 1–6 s hitches: remapping of the game's memory (11/26 08:13 run)

The module `crates/victory-hle/src/modules/xmem.rs` gained a line every 5 s, `title memory remapping in the last ... s`, which splits remapping time into: calls to Windows, commit, notices to the GPU and remapped ranges.

**What the run showed:**
- Outside the hitches, remapping costs ~5–46 ms every 5 s.
- In the hitch bursts, the game makes 7,000–11,400 calls in 5 s, adding up to **4.3–4.9 s**.
  - Almost all of it is calls to Windows (`VirtualFree` to split placeholders, `MapViewOfFile3`, `UnmapViewOfFile2`): about 0.5 ms each.
  - Commit costs only 2–21 ms and the notices to the GPU 70–210 ms.
- The biggest frames add up to ~1.0–1.3 s of remapping each.

**Fix 1, `VICTORY_REMAP_SAME`** (on by default; `=0` goes back to the old behaviour):
- A remap that asks for exactly what the range already shows (same section, same page, address by address) is now left as is.
- This also applies to unmapping what is already unmapped and to the GPU page mapping copy (`map_alias`).
- The visible contents are the same, so there is nothing the GPU or an in-place read could see differently.
- The log line counts these calls.
- New test: `mapping_what_is_there_already_leaves_the_views_alone`. All 103 victory-hle tests pass.

If few calls are of this kind, the next step is to go after the cost of the Windows calls themselves. The line now separates unmap time from split+map time.

### Result of fix 1 (11/26 09:47 run) and what came after

- It barely acts: of every ~3,500–12,600 calls in 5 s, only 0 to 4 asked for what was already there. The session had 4,489 frames, 509 over 100 ms, 150 over 250 ms, 30 over 1 s, and the longest hitch lasted 6.6 s. It is the same picture as before.
- The cost is in the calls to Windows: ~1 ms per mapped view (4,440 views in 4.8 s, for example).
- Isolated test (our own program, outside the game): repeats what `map`/`unmap` ask of Windows, that is, 64 KiB views of a 6 GiB SEC_RESERVE section in an 8 GiB placeholder.

  | condition | per remap | worst map |
  |---|---|---|
  | 5,000 views | 8 µs | – |
  | 100,000 views | 9 µs | – |
  | 2 VirtualQuery threads | 12 µs | – |
  | 2 VirtualProtect threads | 16 µs | 1.0 ms |
  | 1 GetWriteWatch thread over 1 GiB | 17 µs | 5.3 ms |

  In other words, the calls are cheap. In the game they cost ~100 times more, which points to waiting for the process's address space (the VirtualProtect guards, GetWriteWatch on the GPU side, page faults) or for a free core.
- The `title memory remapping` line now shows how long the thread ran in these calls (`QueryThreadCycleTime`) against wall-clock time, and separates placeholder split time. With this the next run tells which of the two is the cause.
- In the heavy areas, the steady 6 fps does not come from remapping, which costs only 8.6 ms per frame there. In the typical 156 ms frame:
  - the submitting thread works 110 ms (list steps 54, write-watch 15, card_before 10, updates 9);
  - the copy-back takes 42 ms (48 MB) and the copies 39 ms (110 MB);
  - recording the game's list calls adds up to 483 ms of CPU over 51,641 calls (~9 µs each).

### Performance build (`perf`)

`scripts/build-perf.ps1 -Cpu znver3` produces `target\perf\victory.exe`:
- release without the `diagnostics` feature;
- fat LTO and one codegen unit (`[profile.perf]` in Cargo.toml);
- code for Zen 3.

To play with it: `scripts\play-nvidia.ps1 -Build perf -Exe ...`. The frame pacing line, the remapping line and the framelog stay in the log, because they do not depend on `diagnostics`.
