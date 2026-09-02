# HyprCube

HyprCube is a Rust-native settings application for [Hyprland](https://hyprland.org/). It renders its own Wayland interface with Smithay Client Toolkit, tiny-skia, and cosmic-text; it does not use GTK, Qt, or Electron.

## Platform requirements

Building requires Rust on Linux. Using the desktop application requires a Wayland session. Hyprland is required for Hyprland IPC features such as live settings previews and monitor control; the application degrades where that IPC socket is unavailable.

## What it provides today

- Hyprland and Input settings backed by a structure-preserving `hyprland.conf` parser.
- Appearance and palette editing with built-in palette presets.
- A native Wayland window, sidebar navigation, form widgets, and panel implementations for Display and About.
- Optional HyprDeck and HyprSaver panels when their configuration files are present.
- Live Hyprland preview with an undo/revert path for Hyprland-backed form edits.

The Display implementation has different safety characteristics from ordinary forms: when its event path is exercised, it applies monitor changes through Hyprland IPC immediately. Read the [runtime-safety guide](docs/runtime-safety.md) before manually exercising the GUI.

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

`install.sh` builds a release binary, invokes `sudo`, and installs it to `/usr/local/sbin/hyprcube`. Inspect and run it only when that privileged system-wide installation is intended.

## Documentation

- [Agent guidance](AGENTS.md)
- [Documentation index](docs/README.md)
- [Development guide](docs/development.md)
- [Architecture guide](docs/architecture.md)
- [Runtime safety guide](docs/runtime-safety.md)
- [Architecture decisions](docs/decisions/README.md)
- [MIT License](LICENSE)
