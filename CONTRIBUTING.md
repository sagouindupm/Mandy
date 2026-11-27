# Contributing to Sagouindupm | VIctory

There is a single goal: **run the game as well as possible on a low-end PC, Series S profile** (8 GB
card, mid-range CPU), with a picture as close to the console as possible. Today: ~4-5 frames per second
on an RTX 3050. Target: 15.

## Get going in 10 minutes

1. Read [docs/test-log.md](docs/test-log.md): what was measured, kept, rejected, and the
   **"To try"** list.
2. If you work with an AI, give it [AGENTS.md](AGENTS.md) first.
3. Build: `cargo build --release -p victory` (Rust toolchain in `rust-toolchain.toml`, MSVC tools and
   Windows SDK).
4. Launch with `series-s\Sagouindupm.bat` (or `series-s\launch_series_s.bat`).
5. **NVIDIA cards**: NVIDIA Control Panel → Manage 3D settings → **Shader Cache Size → Unlimited**.
   Otherwise, once the default 4 GB are full, every new build recompiles its shaders at every launch and
   looks much slower (see `docs/test-log.md`).
6. Set up your measuring bench: [scripts/bench/run.sh](scripts/bench/run.sh) (variables to fill in at
   the top of the file) and a copy of your save at the measured place.

## Sharing the work (nobody steps on anyone's toes)

1. **One task = one *issue*.** Before starting, open an issue (or comment on the existing one) titled
   `[in progress] <the task>` with the relevant row of `docs/test-log.md`. If someone has already taken
   it, pick something else, or offer in the issue to share the work.
2. **One person per area at a time**: two PRs touching the same big file get in each other's way. The
   areas:

   | Area | Files |
   |---|---|
   | Submission, flights, copy back, waits | `crates/victory-gpu/src/host.rs`, `queue.rs`, `queue2.rs` |
   | Descriptor heaps, buffer pass | `crates/victory-gpu/src/descriptor/host.rs`, `draw.rs` |
   | Copies of title memory | `crates/victory-gpu/src/shadow.rs`, `plaincopy.rs`, `watch.rs` |
   | Textures | `crates/victory-gpu/src/texture.rs`, `resource.rs` |
   | Shader translation | `crates/victory-shader/` |
   | Xbox services, input, saves | `crates/victory-hle/` |
   | Launchers and settings | `series-s/` |
   | Measurements on other machines | `docs/test-log.md` only |

3. **An issue with no news for 3 days is free.** Write where you stand, even to say "abandoned, here is
   why": that is a result.
4. **Branches**: `perf/…` (speed), `fix/…` (picture or crash), `re/…` (reverse engineering), `docs/…`.
   Always start from an up-to-date `main`.

## Rules

1. **No direct push to `main`.** A pull request from a branch or a fork, reviewed and merged by the
   maintainer only.
2. **One pull request = one topic**, with a title that says what it changes.
3. **Measurements**, made as in [docs/test-log.md](docs/test-log.md): median before/after over at least
   two alternating runs, the machine (card, CPU, RAM), the place in the game. For a picture fix: a
   before/after screenshot.
4. **The result goes into `docs/test-log.md`** in the same PR: a row in "Kept" or "Rejected", and an
   update of "To try". A documented failed test is worth a PR.
5. **A new setting = a `VICTORY_…` variable**, documented in a comment where it is read, and off by
   default until it is measured.
6. **Nothing from the game**: no files, no dumps, no raw logs, no captures of game code. No patch that
   modifies the game's executable on disk.
7. **License**: GPL-3.0-or-later. No GPL-2.0-only code (XCL, the original Mandy) or code under an
   incompatible license.
8. **Credits**: if you reuse someone's work, keep their commit and name (`git cherry-pick -x`) or name
   them in the message (`after <author> <commit>`). Never reattribute someone else's work.
9. **Contributors**: add yourself to the README's "Contributors" table in your PR (GitHub handle and what
   you brought). A measurement or a bug report counts too.
10. **AI**: welcome; say so in the PR description ("AI-assisted: …").

## Before opening the PR

- `cargo build --release -p victory` passes.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test -p <crate touched>`
  pass.
- The game starts with `--console series-s` and reaches the world.
- `docs/test-log.md` is up to date.
