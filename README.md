# HyprCube

A Rust-native GUI settings panel for [Hyprland](https://hyprland.org/) — no GTK, Qt, or Electron.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      hyprcube                           │
│              (binary: CLI, config, app loop)             │
│                                                         │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│   │ hyprcube-core │  │hyprcube-panels│  │hyprcube-     │ │
│   │              │  │              │  │  hyprconf    │ │
│   │ color        │  │ Hyprland     │  │              │ │
│   │ palette      │  │ Appearance   │  │ hyprland.conf│ │
│   │ layout       │  │ Display      │  │ parser with  │ │
│   │ render       │  │ Input        │  │ roundtrip    │ │
│   │ widget       │  │ HyprDeck     │  │ fidelity     │ │
│   │ ipc          │  │ HyprSaver    │  │              │ │
│   │ undo         │  │ About        │  │              │ │
│   └──────────────┘  └──────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| UI toolkit | Custom (tiny-skia + cosmic-text) | Visual consistency across HyprCube/HyprDeck/HyprSaver ecosystem |
| Wayland integration | smithay-client-toolkit | Direct Wayland, no display server abstraction layer |
| Config editing | Hybrid: file parser + IPC | Parser for disk persistence, IPC for instant live preview |
| Config parser | Structure-preserving | Roundtrip hyprland.conf without losing comments or formatting |
| Preview model | Immediate via IPC + undo stack | Changes appear instantly; reverted if user cancels |
| Theming | Centralized palette.toml | HyprCube writes palette; sibling apps read it |
| Unavailable apps | Graceful degradation | Panels for uninstalled apps are hidden, not disabled |
| App lifecycle | On-demand (no daemon) | Launches when needed; `--daemon` reserved for future |

## Ecosystem — Palette Sync Flow

```
┌───────────┐   writes    ┌──────────────────────────────────┐
│ HyprCube  │────────────▶│ ~/.config/hypr/                  │
│ (settings)│             │   hyprcube-palette.toml          │
└───────────┘             └──────────────┬───────────────────┘
                                     │ reads
                          ┌──────────┴───────────────┐
                          │                          │
                    ┌─────▼─────┐            ┌──────▼──────┐
                    │ HyprDeck  │            │ HyprSaver   │
                    │ (launcher)│            │ (lockscreen) │
                    └───────────┘            └─────────────┘
```

## Panels

| Panel | Description |
|-------|-------------|
| **Hyprland** | General settings: gaps, borders, layout, resize behavior |
| **Appearance** | Palette colors, fonts, border radius, opacity |
| **Display** | Monitor configuration (future: drag-and-drop layout) |
| **Input** | Keyboard layout, mouse sensitivity, touchpad settings |
| **HyprDeck** | Dock icon size, position, auto-hide (shown if installed) |
| **HyprSaver** | Lock timeout, lock-on-sleep, background (shown if installed) |
| **About** | System info: versions, kernel, hostname |

## Build

```bash
cargo build --release
```

The binary is at `target/release/hyprcube`.

## Usage

```bash
hyprcube                    # launch settings panel
hyprcube --panel input      # jump directly to Input panel
hyprcube --reset-config     # delete config and exit
hyprcube --help             # show CLI options
```

## Development

```bash
cargo check                          # type-check workspace
cargo test                           # run all tests
cargo test -p hyprcube-hyprconf      # test just the parser
cargo clippy --workspace             # lint
cargo run -p hyprcube                # run the app
```

## Status / Roadmap

**Done:**
- Workspace structure with 4 crates
- Core primitives: color, palette, layout, render, widget (Label, Toggle), IPC, undo
- Structure-preserving hyprland.conf parser with full roundtrip fidelity
- SettingsPanel trait and 7 panel implementations
- Binary shell: config, CLI, panel registry, preview engine

**Next:**
- SCTK Wayland window and event loop
- Remaining widgets: Slider, Dropdown, TextInput, ColorPicker, ScrollArea, TabBar
- Sidebar navigation UI
- Wire Hyprland panel fields to widgets with IPC preview and save
- Palette editor with live sync to HyprDeck/HyprSaver
- Display panel monitor drag-and-drop
- HyprDeck panel auto-generated from module schemas

## License

TBD
