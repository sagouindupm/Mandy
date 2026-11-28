# Changelog

VIctory S, the downloadable package (`VIctory-S-<version>.zip` on the
[Releases](https://github.com/sagouindupm/Mandy/releases) page): the launcher `VIctory S.exe`, the engine
`engine\victory.exe` and the Series S settings. Measurements behind each change: [docs/test-log.md](docs/test-log.md).

## 0.3.1

About 3 ms faster and no more sky through moving clothes: in the boat mission on an RTX 3050 8 GB,
**68-69 ms per frame** against 71 ms for 0.3.0.

### Faster

- **GPU-read-only memory no longer compared on the card** (`VICTORY_GPU_RO_SKIP=1` in `series-s\reglages.json`,
  after PR #1014 by mikael-jarvinen): 68-69 ms per frame instead of 71 on the boat route.

### Fixed

- **Sky through moving clothes** (boat cutscene): a depth plane a shader wrote is now drawn right after that shader
  (`VICTORY_DEPTH_PLANE_NOW=1` in `series-s\reglages.json`, after trustno1-coder/VIctory-NVIDIA PR #1018 by
  invismann). No fps cost.

### Known issues

- Some faces in the prologue cutscenes can show a green/purple mosaic.
- Small confetti squares can appear in the sky.

## 0.3.0

Faster and cleaner: in the boat mission on an RTX 3050 8 GB, **69-72 ms per frame (~14 fps)** against 74-81 ms for
0.2.0 (two clean rounds each), and the worst frame 145-175 ms instead of up to 1.8 s.

### New

- **Keyboard and mouse laid out as on PC**: WASD, mouse camera, right button aims, left button fires, Shift
  sprints, Space jumps, F enters vehicles, Tab opens the weapon wheel, M the map, H the phone. In a vehicle
  (F, or V by hand; the window's title shows `[driving]`): W accelerates, S brakes, Space is the handbrake, the
  mouse buttons aim and fire from the vehicle. AZERTY and QWERTZ by key place, or by printed letter (Settings,
  "Keyboard layout"). Keys can be rebound (`VICTORY_KEYMAP`); the Help page lists the layout. A controller
  still works alongside.

### Faster

- **2 MB mapping sections** (`VICTORY_SECTION_MB=2` in `series-s\reglages.json`, after trustno1-coder's PR #44):
  69-72 ms per frame instead of 74-81, worst frame 145-175 ms instead of up to 1.8 s.

### Fixed

- **Textures**: placed views no longer read padding words as texture data, so no more rainbow confetti on surfaces
  (after trustno1-coder #58); BC7 blocks using dictionary entry 0 decode; a texture refused for lack of room on the
  card is asked for again instead of staying black (after fxceless); a placed view aliased by another resource is
  kept pinned; views written by the card are copied to their aliases. With fixes from trustno1-coder,
  fxceless and Vice-Tay.

### Known issues

- Some faces in the prologue cutscenes can show a green/purple mosaic.
- The sky can show through clothes in the boat scene.

## 0.2.0

### New

- **Frame generation (FSR 3)**, in Settings, off by default: about twice the frames shown. The input lag stays
  that of the real frames, so it is best above ~30 real fps; it uses ~1 GB of video memory.
  - NVIDIA RTX 20/30 and GTX 16: AMD FSR 3 frame generation behind NVIDIA DLSS-G's calls, through
    [dlssg-to-fsr3](https://github.com/Nukem9/dlssg-to-fsr3) (Nukem9, GPL-3.0), built from source at commit
    0e253f63 and shipped in `fg\` with its licence and build notes. RTX 40/50: NVIDIA's own DLSS Frame Generation.
    Not offered on AMD and Intel cards yet.
  - NVIDIA's Streamline 2.14.16 files are fetched from NVIDIA's own release when the option is turned on (or with
    "Get frame generation files"), each DLL checked against its SHA-256, into `%LOCALAPPDATA%\Sagouindupm\framegen`.
    They are never in the zip (NVIDIA's licence).
  - While it is on, the card memory settings make room for it (card buffers 768 MB at most, texture room 90 %):
    measured on an RTX 3050 in the boat mission, 91 ms per real frame with 7 card refusals and textured
    characters, against 189 refusals (black objects) without that.
  - The game window's title shows `11 fps · ~22 shown`.
  - Engine: DLSS-G through Streamline from every upscale, after trustno1-coder/VIctory-NVIDIA and
    pepezin-ai/VIctory-NVIDIA (`VICTORY_FRAMEGEN`, `crates/victory-gpu/src/ffx/framegen.rs`).
- **Low video memory mode**, in Settings, for 6 GB cards or black screens (smaller textures, fewer card buffers);
  recommended, never forced, on cards with 6 GB or less.
- Help page and README: advice for 6 GB cards and laptops (Windows graphics setting "High performance" for
  `VIctory S.exe` and `engine\victory.exe`, programs to close, the low video memory mode), frame generation, how to
  send your log.
- The console launcher (`series-s\dev\Sagouindupm.bat`) has both options too ([7], [8]); both launchers read them
  from `series-s\reglages.json`. With both options off the game gets exactly the settings of 0.1.0.

### Packaging

- The zip gains `fg\` (dlssg-to-fsr3) and `series-s\framegen.ps1`; its check allows only those two named DLLs under
  `fg\` and still refuses any NVIDIA DLL, by name and by content.
- 0.2.0 is packaged locally (`scripts\package-release.ps1 -Fsr3Build <dlssg-to-fsr3 build>`): the release workflow
  cannot build dlssg-to-fsr3 yet and stops with that explanation.

## 0.1.0

First downloadable version (27 November 2026).

- **VIctory S launcher** (Tauri + React): step-by-step setup ("Prepare everything": the game, the shaders, the
  pipelines of the shared list), Play, Settings, Help, in English or French; the hardware profile (CPU threads,
  card memory, RAM, vendor) picked automatically from the PC; DLSS modes on RTX cards with your own Streamline
  folder; "Give feedback" opens a GitHub form filled in with your setup.
- **Engine** on the trustno1-coder/VIctory-NVIDIA lineage tuned by Maykzera, Proudnoob4's `nvidia-hybrid-v2`,
  edupode and community picture fixes: ~10 fps in the boat mission on an RTX 3050 8 GB (84-97 ms per frame).
- **Shared pipeline list** (`series-s\pipelines\series-s-rtx.txt`, 3,535 pipelines, no game code): the first
  session is smooth until after the boat mission.
- Built for any x86-64 CPU with AVX2, no Visual C++ runtime to install; nothing from the game in the zip.
