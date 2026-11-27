# Device slots 0x1c8 and 0x218: pipelines the title deserializes

Reverse engineering by Sagouindupm | VIctory, 2026-11-26 (RTX 3050, Series S, title 1.0.2.1).
Data collected with `VICTORY_DUMP_UNKNOWN_PIPELINES=<folder>` (nothing from the game in this repository).

## What they are
- **0x218** = `DeserializeComputePipelineStateX` (as Marvisak/VIctory#2396 found for the AMD upscaler).
  Description: `root, (packet, 0x1c), (hash, 0x14), (GFX10 code, n), (metadata, m), ...`.
- **0x1c8** = its graphics version (`DeserializeGraphicsPipelineStateX`, assumed name). No fork implements it.
  Measured: the ~24,500 calls all land within 2 s at load, about 12,000 distinct pipelines requested
  18 times each, no repeat per frame. It is the game's precompiled pipeline cache; it then falls back
  to regular creation (DXIL), which we translate. Expected gain from implementing it: load time and
  shader compilation, not the picture.

## 0x1c8 description (64-bit words)
| word | content |
|---|---|
| 0 | root signature (our object) |
| 1, 2 | register packet, 0x184 to 0x200 bytes |
| 3, 4 | vertex input format (names `POSITION`, `COLOR`, `TEXCOORD` in plain text), 0x50 to 0xbc |
| 5-8 | stage 0 = **pixel shader**: GFX10 code, size; metadata, size |
| 9-12 | stage 1 = **vertex shader** (exports `pos0`, `param0+`) |
| 13-16 | stage 2 = vertex stage when stage 1 is empty (seen with a 19 KB pixel shader) |

Stages are recognized by their exports (`exp mrt0` for the pixel shader, `exp pos0`/`param` for vertex stages).

## Stage metadata
Same format as FSPX `kind 7` in `fspx_program` (Marvisak main, `native.rs`), version 2 instead of 0x403:
`u32 2, u32 size-0xc, u32 code words, u8 0x40, u8 first user-data dword, u8 number of 0x41 records, u8 0`,
then 32-byte records (0x41 resource read, 0x42 view declaration, 0x43 hash).

## What is missing to create them
1. The FSPX/ACCC format in our translator (port `fspx_program`).
2. The register packet: SPI_PS_INPUT_*, vertex shader outputs, colour target formats (the code already exports in the target's format, without the FSSM export fixes).
3. Fabricated input/output signatures (the game passes no DXIL).

## White water (investigation of 2026-11-26)
Tools: `VICTORY_CAPTURE_AT=<seconds>` (capture at a given time; cutscenes follow the clock),
`VICTORY_DUMP_TARGETS=1`, raw values of small targets in the log (`diagnostics` build).
- HDR lighting is correct (the prison yard sunset is right before post-processing);
  exposure is 2^-12.9 and the sky, trees and characters are well exposed.
- Only the water is white, already in the lit HDR target: the sunlit sand shows under the water, with no absorption.
- The water compute `cad2edef5ecea3ba` writes the turquoise colour (R11G11B10 1280x720), riddled with black bands,
  and no pass binds it as an input in the same frame (a read by index, bindless, is still possible
  and does not show up in the list of operations).
- The only skipped draws: the ray tracing builder's steps (`VICTORY_BVH_BUILDER` off).
