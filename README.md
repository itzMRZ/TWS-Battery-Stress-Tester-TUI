# tws-tester

[![CI](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/actions/workflows/ci.yml/badge.svg)](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/itzMRZ/TWS-Battery-Stress-Tester-TUI)](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Plays audio into a Bluetooth headset or earbuds until playback actually stops, then writes a report (battery charts, CSV, events) to `Documents/tws-tester/`.

> Linux is supported. Windows is experimental: it can only capture a probe, not run a soak.

## Install

Linux (x86_64):

```
curl -sSfL https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.sh | sh
```

Windows (x86_64, experimental):

```
irm https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.ps1 | iex
```

This installs to `~/.local/bin/tws-tester`. Run `tws-tester --update` later to pull the latest release.

## Use

Pair and connect your headset, then run:

```
tws-tester
```

Press `?` in the app for keys. Reports stay on disk if the process exits.

| Command | Purpose |
| --- | --- |
| `tws-tester` | Terminal UI |
| `tws-tester probe` | Capture host and device facts |
| `tws-tester --history` | Open saved reports |
| `tws-tester --update` | Install the latest release |
| `tws-tester --version` | Print the version |

Battery percent shows up automatically for supported brands (soundcore, Samsung, and Apple are verified; several others have a parser but are not yet verified on real hardware). Anything else still runs, it just will not show a battery curve. [CONTRIBUTING.md](CONTRIBUTING.md) has the full list.

## Contributing

Issues and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) is the start.

## License

[MIT](LICENSE)
