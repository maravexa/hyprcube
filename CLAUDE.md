# CLAUDE.md — HyprCube Project Context

## Project Overview

HyprCube is a Rust-native GUI settings panel for [Hyprland](https://hyprland.org/), part of an ecosystem alongside HyprDeck (app launcher/dock) and HyprSaver (screensaver/lock screen). It uses no GTK, Qt, or Electron — rendering is handled entirely via smithay-client-toolkit for Wayland integration, tiny-skia for 2D drawing, and cosmic-text for text shaping. HyprCube manages Hyprland configuration, theming (palette), and settings for sibling apps through a custom widget toolkit.

## Architecture

```
crates/
├── hyprcube-core/       # Color, palette, layout, render, widget trait, IPC client, undo stack
├── hyprcube-hyprconf/   # Structure-preserving hyprland.conf parser with roundtrip fidelity
├── hyprcube-panels/     # SettingsPanel trait + 7 panel implementations
└── hyprcube/            # Binary: config, CLI, panel registry, preview engine, app entry point
```

## Key Design Decisions

- **Custom widget toolkit** over iced/egui for visual consistency across the HyprCube/HyprDeck/HyprSaver ecosystem
- **Hybrid hyprland.conf strategy**: file parser (`hyprcube-hyprconf`) for persistence, Hyprland IPC for live preview
- **On-demand app** (no daemon); `--daemon` flag is reserved for future use
- **Immediate IPC preview** with undo stack; changes are written to disk only on explicit Save
- **Palette authority**: HyprCube writes `~/.config/hyprcube/palette.toml`, sibling apps read it
- **Graceful degradation**: panels for uninstalled apps (HyprDeck, HyprSaver) are hidden automatically
- **Structure-preserving parser**: hyprland.conf roundtrips without losing comments, blank lines, or ordering

## Config File Locations

All paths respect `$XDG_CONFIG_HOME` (default `~/.config`).

| File | Path | Format |
|------|------|--------|
| HyprCube config | `~/.config/hyprcube/config.toml` | TOML |
| Palette | `~/.config/hyprcube/palette.toml` | TOML |
| Hyprland | `~/.config/hypr/hyprland.conf` | Custom (NOT TOML) |
| HyprDeck | `~/.config/hyprdeck/config.toml` | TOML |
| HyprSaver | `~/.config/hyprsaver/config.toml` | TOML |

## Hyprland IPC

Socket path: `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`

HyprCube uses the **command socket only** (not the event socket).

Key commands:
- `j/monitors` — query monitors as JSON
- `j/getoption <name>` — query a single option value as JSON
- `/keyword <key> <value>` — apply a setting change (live preview)
- `/reload` — reload Hyprland config from disk
- `dispatch <cmd>` — dispatch a Hyprland command
- `configerrors` — query config parse errors

## Development Commands

```bash
cargo check                          # type-check workspace
cargo test                           # run all tests
cargo test -p hyprcube-hyprconf      # test just the parser
cargo run -p hyprcube                # run the app
cargo run -p hyprcube -- --help      # CLI usage
cargo clippy --workspace             # lint
```

## Dependency Constraints

- `hyprcube-core` must NOT depend on hyprdeck-modules or the hyprdeck binary
- Can depend on `hyprdeck-core` (library crate) for module config schemas
- All external HTTP via reqwest with `rustls-tls` (no OpenSSL)
- Prefer workspace dependency versions declared in the root `Cargo.toml`

## What's Implemented

- [x] Workspace + root Cargo.toml with workspace dependencies
- [x] `hyprcube-core`: color, palette, layout, render, widget (Label, Toggle), IPC client, undo stack
- [x] `hyprcube-hyprconf`: structure-preserving parser with roundtrip, query/mutation API, 23 tests
- [x] `hyprcube-panels`: SettingsPanel trait + 7 panels (Hyprland, Appearance, Display, Input, HyprDeck, HyprSaver, About)
- [x] `hyprcube` binary: config, CLI (`--panel`, `--daemon`, `--reset-config`), panel registry, preview engine
- [ ] SCTK Wayland window + event loop
- [ ] Widget rendering (Slider, Dropdown, TextInput, ColorPicker, ScrollArea, TabBar)
- [ ] Sidebar navigation UI
- [ ] Hyprland panel: wire fields to widgets, IPC preview, save
- [ ] Palette editor with live sync to HyprDeck/HyprSaver
- [ ] Display panel: monitor layout drag-and-drop
- [ ] HyprDeck panel: auto-generate UI from module config schemas

## Code Style

- Use `thiserror` for all error types
- Use `tracing` for logging (not `println!` or `log`)
- All config IO functions respect `XDG_CONFIG_HOME`
- Serde derives on all config structs
- Tests go in `#[cfg(test)] mod tests` in each file
- Workspace resolver = "2"
