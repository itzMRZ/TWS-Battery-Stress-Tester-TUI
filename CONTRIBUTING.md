# Contributing

Issues and pull requests are welcome. This project is MIT. Do not copy GPL or AGPL code into the tree. Reading those projects for opcodes and frame layouts is fine.

Build and test on Linux. Bluetooth and system audio need to work for a live run.

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --lib -- --skip live_
```

`cargo build --release` writes `target/release/tws-tester`. `live_*` tests talk to specific devices and are skipped in CI; only run them when that hardware is connected.

## Layout

| Path | Role |
| --- | --- |
| `src/ui/` | Terminal UI |
| `src/host/` | Bluetooth and audio on the host |
| `src/cells/` | Battery parsers, one brand group each |
| `src/brand/` | Name / UUID detect, logos |
| `src/death.rs` | When a run ends |
| `src/pack/` | Report files on disk |
| `src/probe.rs` | `tws-tester probe` |
| `src/cli.rs` | `--version`, `--history`, `--update` |
| `assets/logo/` | Brand SVGs |
| `docs/adr/` | Why these product rules exist |
| `docs/families.md` | Protocol notes and licenses not to copy |

## Words

Use these in code, reports, issues, and commits so the tree stays consistent:

| Word | Meaning |
| --- | --- |
| Device | One Bluetooth audio sink. Two buds are still one Device. |
| Cell | One named battery (`left`, `right`, `case`, `pack`, `pair`). Missing stays unknown. Never invent a cell. |
| Family | One adapter for a brand group (Soundcore includes Anker). Not a per-model branch. |
| Soak | One timed run until death or stop. |
| Death | Playback gone after grace. Not a reported 0% while audio still flows. |
| Support pack | The report folder for a soak. |
| Probe | Host and Device facts. Not a soak. |
| Brand | Name on the box, from advertised name and vendor UUIDs. Not the chip vendor. |

## Hardware

See the [README](README.md#hardware) list. Verified Linux soaks pin MACs in `src/host.rs`. Windows can probe; it cannot play or soak.

## Issues

Use a [GitHub issue](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/issues/new/choose).

- **Bug:** OS, `tws-tester` version, what happened. A report folder or probe folder helps.
- **Device or family:** advertised name, OS, and a `tws-tester probe` folder.

Security: [SECURITY.md](SECURITY.md).

## Adding a Family

1. `tws-tester probe` on a connected Device. Keep that folder.
2. Prefer an MIT or Apache write-up or a first-party spec. `docs/families.md` lists nearby projects and which licenses must not be copied.
3. Parser in `src/cells/<family>.rs`. Dispatch is `Family::cell_transport()`, never a model name.
4. Headphones report `pack` when that is all they send. Do not invent left/right.
5. Unit tests that do not need the radio. Add a `live_*` test only if that Device is on hand and its MAC is pinned.

Logos: [assets/logo/README.md](assets/logo/README.md).

## Pull requests

Small diffs. Match the words above. `cargo test --lib -- --skip live_` green. No GPL.

## Release

Keep `Cargo.toml` `version` and the git tag in sync (for example `v0.1.1`). The Release workflow refuses a mismatch, then attaches Linux and Windows binaries, a `.sha256` file for each, and the install scripts.

Users install from `releases/latest`. Do not point install docs at `raw/main`.
