# Picture defaults to carry into the launcher settings

`series-s/reglages.json` lives on another branch. Add these to its environment block (and to
`$SeriesS` in `series-s/Sagouindupm.ps1` where that one is used) when this branch is merged:

| Variable | Value | Why |
|---|---|---|
| `VICTORY_WAVE_WIDE_DIGESTS` | `7099a57fded142613b44f410463d6be6` | Water volumetrics `_LocalLights` (compute `6f225b641c5b0708`) built in the wave-wide form instead of refused (validated 27/11/2026). |
| `VICTORY_SKIP_PIPELINES` | drop `6f225b641c5b0708` from the list | Otherwise the program above stays skipped. Keep the others (`731a…`, `5dc3…`, `926c…`, `1f1f…`, `21f7…`, `7c9a…`). |
| `VICTORY_VRAM_RESERVE` | `max` | Reserves the card's memory for us (validated 27/11/2026). |
| `VICTORY_AMBIENT_SETTLE` | `250,3` | Interior white/green-brown veil: the stand-in's slow mean settles by the clock (Dathinel #15). Off in code until set. Needs `VICTORY_AMBIENT` on (the launcher sets `0.8`). |

Nothing to set for the other fixes on this branch:

- BC7 rotation only in modes 4 and 5: always on, no switch.
- Zeroed pooled texture staging: on by default; `VICTORY_STAGING_ZERO=0` turns it off for an A/B.
- Violet/magenta probe filter: always on whenever `VICTORY_AMBIENT` is on.
