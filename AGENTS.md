# Agents

This repository is **tws-tester**: a Rust terminal program that plays audio into a Bluetooth audio Device until playback is gone, and writes a support pack on disk. Linux is the tested host. Windows probe exists; soak and playback do not.

Read [CONTRIBUTING.md](CONTRIBUTING.md) for humans. This file is the short map for any coding agent working in the tree.

## Commands

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --lib -- --skip live_
```

`live_*` tests require specific MACs in `src/host.rs` (soundcore P30i, Samsung Galaxy Buds2 Pro, Apple AirPods Pro). CI skips them. Do not run them without that hardware.

Users install with the README one-liner from `releases/latest` (SHA-256 checked). Contributors: `cargo build --release`. Do not point install docs at `raw/main`.

## Layout

- `src/main.rs`: TUI, `probe`, `--history`, `--update`, `--version`
- `src/cli.rs`: history folder open; GitHub release update with SHA-256 check
- `src/ui/`: deck, prep, soak, archive
- `src/host/`: BlueZ, Pulse/PipeWire, RFCOMM, AAP, Windows probe logs
- `src/cells/<family>.rs`: battery parsers, one Family each
- `src/brand/`: advertised name and UUID detect; `marks/` paints logos
- `src/death.rs`: stop rules
- `src/pack/`: HTML, CSV, JSONL, TXT under `Documents/tws-tester/`
- `src/probe.rs`: probe capture
- `src/alias.rs`: local nickname; pack still stores name and address
- `assets/logo/`: `{slug}-icon.svg` / `{slug}-logo.svg`, rasterized by `build.rs`
- `docs/adr/`: design choices
- `docs/families.md`: protocol facts and GPL-avoidance

## Words

Use these in code, comments, packs, and commit messages:

| Term | Meaning |
| --- | --- |
| Device | One Bluetooth audio sink. Two buds are still one Device. |
| Cell | One named battery (`left`, `right`, `case`, `pack`, `pair`). Missing stays unknown. Never invent a cell. |
| Family | One adapter for a brand group (Soundcore includes Anker). Not a per-model branch. |
| Soak | One timed run until death or stop. |
| Death | Playback gone after grace. Not a reported 0% while audio still flows. |
| Support pack | The report folder for a soak. |
| Probe | Host and Device facts. Not a soak. |
| Brand | Name on the box, from advertised name and vendor UUIDs. Not the chip vendor. |

## Rules

- Family adapters, not per-model branches.
- MIT only in the tree. GPL/AGPL projects may be read for opcodes; do not copy their code. See `docs/families.md`.
- Death is playback gone (`src/death.rs`, `docs/adr/0003-death-not-percent.md`). The TUI process is the soak (`docs/adr/0002-tui-is-the-soak.md`).
- A broken logo is a blank slot, not a failed build.
- Do not document `cargo install tws-tester` until the crate is on crates.io.
- Do not add network services. `--update` is an outbound fetch of the GitHub latest release; it is not a server.
- Cargo.toml `version` and the git tag (for example `v0.1.1`) must match. Release assets include `.sha256` files. Never skip that check in install.sh or `--update`.

## Hardware (verified vs not)

Same three tiers as the [README](README.md#hardware):

- Verified on real hardware: soundcore P30i, Samsung Galaxy Buds2 Pro, Apple AirPods Pro
- Parser exists, not yet verified on real hardware: Beats, Sony, Nothing/CMF, Bose, Oppo/OnePlus/Realme
- Recognized by name only, no battery parser: Xiaomi, Vivo, Google, Huawei, Honor, JBL, Jabra, Sennheiser, Edifier, Marshall

## Changing behavior

Match existing modules. New Family: probe folder, `src/cells/<family>.rs`, `Family::cell_transport()`, tests without the radio. New logo: `assets/logo/README.md`.
