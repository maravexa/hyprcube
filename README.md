# HyprCube

[![CI](https://github.com/maravexa/hyprcube/actions/workflows/ci.yml/badge.svg)](https://github.com/maravexa/hyprcube/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/hyprcube)](https://crates.io/crates/hyprcube)
[![AUR](https://img.shields.io/aur/version/hyprcube)](https://aur.archlinux.org/packages/hyprcube)

HyprCube is a Rust-native settings application for [Hyprland](https://hyprland.org/). It renders its own Wayland interface with Smithay Client Toolkit, tiny-skia, and cosmic-text; it does not use GTK, Qt, or Electron.

## Platform requirements

Building requires Rust on Linux. Using the desktop application requires a Wayland session, and configuration history requires the `git` executable. Hyprland is required for Hyprland IPC features such as live settings previews and monitor control; the application degrades where that IPC socket is unavailable.

## What it provides today

- Hyprland and Input settings backed by a structure-preserving `hyprland.conf` parser.
- Appearance and palette editing with larger default text, built-in palette
  presets, and sliders whose synchronized numeric fields accept exact values.
- Display discovery and resolution, refresh-rate, scale, transform, enablement,
  and layout controls using current Hyprland monitor data.
- A dynamic Hyprpaper panel with per-monitor wallpaper assignments, a visual
  display-layout preview, fit modes, and live apply.
- Optional HyprDeck and HyprSaver panels when their applications or
  configuration files are present. HyprDeck settings follow its selected
  theme; HyprSaver exposes its native options, playlists, a preview launcher,
  and snapshot-friendly palette/playlist/wallpaper overrides.
- An About panel with project, system, license, and dependency attribution.
- Live Hyprland preview with an undo/revert path for Hyprland-backed form edits.
- Git-backed configuration history with older/newer navigation, section-filtered
  diff previews, restoration, and named snapshots for favorite configurations.

The Display and Hyprpaper implementations have different safety
characteristics from ordinary forms: their Apply paths can change the live
desktop through Hyprland IPC. Read the
[runtime-safety guide](docs/runtime-safety.md) before manually exercising the
GUI.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `hyprcube` | Application entry point, CLI, Wayland event loop, shared config lifecycle, and preview orchestration. |
| `hyprcube-core` | Rendering and widget primitives, palette support, Hyprland IPC, undo, and shared XDG config-path helpers. |
| `hyprcube-panels` | Settings-panel contract and panel implementations. |
| `hyprcube-hyprconf` | Structure-preserving `hyprland.conf` parsing and mutation. |

For the dependency and data-flow details, see the [architecture guide](docs/architecture.md).

## Build and inspect

```bash
cargo build --release --locked
cargo run --locked -p hyprcube -- --help
cargo run --locked -p hyprcube -- --version
```

`--help` and `--version` exit before application startup. Running `cargo run -p hyprcube` without either option starts the GUI and is not a routine smoke test. Do not use `--reset-config` unless you intend to delete the HyprCube application config; it has no confirmation prompt.

`--daemon` is recognized but not implemented.

## Install

`install.sh` builds a release binary, invokes `sudo`, and installs it to `/usr/local/bin/hyprcube`. Inspect and run it only when that privileged system-wide installation is intended.

After the initial release, the same binary will also be installable with
`cargo install hyprcube`, from the AUR as `hyprcube`, or from the `.deb`,
`.rpm`, and `.tar.zst` files attached to each GitHub release.

## Documentation

- [Agent guidance](AGENTS.md)
- [Documentation index](docs/README.md)
- [Development guide](docs/development.md)
- [Release guide](docs/release.md)
- [Architecture guide](docs/architecture.md)
- [Runtime safety guide](docs/runtime-safety.md)
- [Architecture decisions](docs/decisions/README.md)
- [MIT License](LICENSE)
