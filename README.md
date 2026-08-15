# tws-tester

[![CI](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/actions/workflows/ci.yml/badge.svg)](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/itzMRZ/TWS-Battery-Stress-Tester-TUI)](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Play audio into a Bluetooth headset or earbuds until the audio actually stops. Each run writes a report folder (charts, CSV, events) under `Documents/tws-tester/`.

> [!NOTE]
> **Linux** is the supported host. Pair and connect the headset first. **Windows** is experimental and can only capture a probe.

## Install

### Linux (x86_64)

```
curl -sSfL https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.sh | sh
```

Installs to `~/.local/bin/tws-tester`. Run the same command again, or `tws-tester --update`, to pull the latest release.

### Windows (x86_64, experimental)

Probe only; playback and reports are not implemented yet.

```
irm https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.ps1 | iex
```

## Run

```
tws-tester
```

Keys live in the app (`?`). Reports stay on disk if the process exits.

| Command | Purpose |
| --- | --- |
| `tws-tester` | Terminal UI |
| `tws-tester --history` | Open saved reports |
| `tws-tester probe` | Capture host and device facts |
| `tws-tester --update` | Latest GitHub release |
| `tws-tester --version` | Version |

## Scope

**In**

- One connected Bluetooth audio device (two buds still count as one)
- A timed run until playback is gone, or until the run is stopped
- Battery percents when this project has an adapter for that brand
- A report folder for every run

**Out**

- Vendor-app features (EQ, firmware, gesture maps)
- A background service
- Treating a reported 0% as the end of the run while audio still plays
- Inventing a left/right battery the device never sent
- Promising every model in a brand from one verified unit

## Hardware

The soak itself (playback and death detection) works with any Bluetooth audio device. Battery reporting depends on having a parser for that brand:

- **Verified on real hardware:** soundcore P30i, Samsung Galaxy Buds2 Pro, Apple AirPods Pro
- **Parser exists, not yet verified on real hardware:** Beats, Sony, Nothing / CMF, Bose, Oppo / OnePlus / Realme
- **Recognized by name only, no battery parser:** Xiaomi, Vivo, Google, Huawei, Honor, JBL, Jabra, Sennheiser, Edifier, Marshall

Anything outside these lists still soaks; it just will not show a battery curve.

## Contributing

Issues and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) is the start. For hardware that is not listed above yet, attach a `tws-tester probe` folder to the issue.

## License

[MIT](LICENSE)
