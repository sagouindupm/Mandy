# Helping VIctory S with an AI assistant

Paste one of these prompts to your AI assistant (Claude, ChatGPT, Codex...) and work with it on a branch of your
fork. Before starting, open an issue titled `[in-progress] <topic>` so nobody works on the same thing twice, and send a
pull request with your numbers and before/after screenshots. [AGENTS.md](../AGENTS.md) and
[docs/test-log.md](test-log.md) hold the rules and everything already tried.

## RTX 20 / 30 and 6-8 GB cards: picture fidelity and speed

```
You are a contributor to VIctory S (https://github.com/sagouindupm/Mandy), a Windows compatibility layer that runs
an Xbox Series game from the player's own copy, tuned for the Series S profile on small PCs (reference: RTX 3050
8 GB, i5-12400F, 32 GB RAM: ~75 ms a frame, 13 fps, in the boat mission). Read AGENTS.md and docs/test-log.md first:
they hold the rules, the code map, how to measure and everything already tried (kept and rejected).

Your role: help me improve PICTURE FIDELITY (a 1:1 Series S picture) and SPEED on my RTX 20/30-class PC, on a branch
of my fork, and prepare a clean PR.

Rules:
- Never commit game files, dumps, captures of game code, logs with paths, or binaries. Never run binaries from
  strangers; build from source. No GPL-2-only code. Keep original authors when porting a commit (cherry-pick -x) and
  credit the source repo in the message.
- Measure properly: own cache folder, launcher "Prepare everything" done, NVIDIA shader cache unlimited, nothing else
  compiling or running while you measure, same scene, at least 2 runs per variant + a control run. Report the median
  ms/frame and p90 from the log lines "fps, ... ms/frame", the RAM/VRAM peak, and screenshots every ~10 s.
- One change per A/B. A picture fix must not cost more than a few ms; a speed change must not degrade the picture.
- Before inventing anything, look for an existing fix in the VIctory forks (trustno1-coder/VIctory-NVIDIA,
  Proudnoob4/VIctory-NVIDIA, fxceless/victory-fork, avatarmanz/VIctory-7900XTX, readmet3xt-lab/foxhound,
  Dathinel/victory-4gb, stellarr-studio/kelvaroq, Marvisak/VIctory and their PRs/issues) and on PortWatch.

Topics nobody is working on yet (pick ONE, open an issue "[in-progress] <topic>" first):
1. Stretched "spaghetti" vertices for a frame or two (gulls, characters): likely RELAY=2 / CARD_STEPS=2 buffer
   timing; see avatarmanz PR #434.
2. Overexposed white light in the convenience-store scene of the prologue.
3. Blocky (16x16 tiles) light on faces in close-ups.
4. White water in cutscenes (no known fix anywhere).
5. Wrong shadows / yellow hair / walls lit in blocks depending on the pipeline library (fxceless PR #1461).
6. Compute threads given wrong ids in 2-D groups (trustno1 PR #30; 221 programs affected).
7. Intro video frames out of order and colour ghosting on TV/phone videos (fxceless 100e05874, a89d3921b).
8. Dark blocks on the water near boats (foxhound LOOPS_WAVE) and squares of clear sky in clouds.
We (the maintainers) work on: GPU cost per command (barriers, indirect draws), memory placement, the launcher.

Deliver: a branch with one commit per change, docs/test-log.md rows with your numbers, before/after screenshots,
your hardware, and a PR description in English.
```

## RTX 40 / 50: push the limits, new launcher profiles, frame generation

```
You are a contributor to VIctory S (https://github.com/sagouindupm/Mandy), a Windows compatibility layer that runs
an Xbox Series game from the player's own copy, Series S profile. Read AGENTS.md and docs/test-log.md first.
Your role: on my RTX 40/50 PC, find the best settings and features for strong cards without degrading the picture,
and turn them into launcher profiles and options, in a clean PR.
Rules: no game files or binaries in commits; build from source; keep original authors when porting (cherry-pick -x);
measure properly (Prepare everything done, own cache, nothing else running, same scene, 2+ runs per variant and a
control; median ms/frame + p90 from the log, VRAM/RAM peak, screenshots); one change per A/B.
Tasks: (1) measure DLSS Quality/Balanced/Performance and render scales; (2) measure VICTORY_CARD_BUFFERS_MB 1536/2048,
VICTORY_CARD_ROOM_FOR_TARGETS_MB and helper threads on your card; (3) add the winning values as a new tier in
series-s/reglages.json "materiel" (card memory / CPU threads) with your numbers in docs/test-log.md, and extend
scripts/check-profils.ps1 and launcher/src-tauri/src/profile.rs tests; (4) optionally port DLSS Frame Generation from
trustno1-coder/VIctory-NVIDIA (fg.rs, F1 menu) with pepezin-ai/VIctory-NVIDIA's constants fix, as a launcher option
shown on RTX 40/50 only.
```
