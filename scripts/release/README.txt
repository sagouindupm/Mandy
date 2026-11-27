VIctory S {VERSION}: quick start
================================

VIctory S runs YOUR copy of an Xbox Series game on PC, with the Series S profile tuned for
low-end PCs. No game file is provided here: you need your own copy.

0. You need the COMPLETE copy of the game: game_scarlett_master.exe with all its archives
   next to it (shaders.rpf, levels.rpf and the other .rpf files, ~130 GB for v1.0.2.1), in its
   own folder (for example C:\Games\GTA6), not loose in Downloads.
1. Unzip this folder wherever you like (not in the game folder).
2. Double-click "VIctory S.exe".
3. Setup tab, "Prepare everything" button: the launcher finds the game
   (game_scarlett_master.exe from your copy), translates the shaders for your card, then
   builds ahead of time the pipelines of the shared list (25 to 40 minutes, only once).
   VIctory itself is already included: nothing to compile.
4. Play tab, PLAY button: the first session is already smooth until after the boat
   mission. Further on, Setup, step 4, "Build" adds the recorded pipelines.

The profile (CPU threads, card memory, RAM) is picked automatically from your PC: see
"Detected profile" in the Settings tab.

Tips
- NVIDIA card: NVIDIA Control Panel > Manage 3D settings > Shader Cache Size > Unlimited.
- 16 GB of RAM: close your browser, Discord and other programs before playing.
- Laptop: plug in the charger, and in Windows Settings > System > Display > Graphics set
  "VIctory S.exe" and engine\victory.exe to "High performance" (the NVIDIA card).
- 6 GB card, or black screens: close your browser, Discord and OBS, and turn on Settings,
  "Low video memory mode".

Frame generation (Settings, "Frame generation (FSR 3)", off by default): about twice the
frames shown; the input lag stays that of the real frames; best above ~30 real fps; uses
~1 GB of video memory. NVIDIA RTX 20/30 and GTX 16: FSR 3 through dlssg-to-fsr3 (fg folder);
RTX 40/50: NVIDIA's own. NVIDIA's files are fetched from NVIDIA when you turn it on.

Something wrong? Send your log: Logs folder (sidebar), the newest partie_....log, attached
to your report on GitHub.
- A CPU with AVX2 is required (Intel 4th generation / AMD Ryzen or newer), and 64-bit
  Windows 10 or 11 with WebView2 (already there on an up-to-date Windows).

Give feedback: Help tab or bottom of the window, "Give feedback". The GitHub form opens
already filled in with your configuration; you read it over and send it yourself.

Contents
- VIctory S.exe              the launcher
- engine\victory.exe         the VIctory engine (Series S profile)
- series-s\reglages.json     the measured settings, the per-hardware profiles
- series-s\pipelines\series-s-rtx.txt
                             the shared pipeline list (no game code)
- series-s\framegen.ps1      fetches NVIDIA's frame generation files from NVIDIA
- scripts\rt_off_hashes.txt  the skipped ray tracing passes
- fg\                        dlssg-to-fsr3 (Nukem9, GPL-3.0), built from source: see
                             fg\README.txt
- LICENSE                    GPL-3.0-or-later

Your settings, logs and screenshots: %LOCALAPPDATA%\Sagouindupm
Source, help and measurements: https://github.com/sagouindupm/Mandy
This project is not affiliated with the game's publisher or developer.
