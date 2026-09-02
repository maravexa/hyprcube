# Architecture

HyprCube is a native Rust Wayland settings application for Hyprland. This
document describes the architecture implemented in the workspace; operational
constraints for interacting with a running desktop are in
[Runtime safety](runtime-safety.md).

## Workspace boundaries

The root `Cargo.toml` defines a four-crate workspace and centralizes shared
dependency versions.

```text
hyprcube-core ───────┐
                     ├──> hyprcube-panels ──┐
hyprcube-hyprconf ───┘                      ├──> hyprcube (binary)
hyprcube-core ──────────────────────────────┤
hyprcube-hyprconf ───────────────────────────┘
```

| Crate | Responsibility |
| --- | --- |
| `hyprcube-core` | Reusable UI and application primitives: color and palette types, geometry, drawing helpers, text rendering, widgets, Hyprland IPC, and undo state. |
| `hyprcube-hyprconf` | Parses and mutates `hyprland.conf` while preserving comments, blank lines, ordering, and nested structure. Serialization normalizes indentation. |
| `hyprcube-panels` | Defines `SettingsPanel`, field metadata, form layout, and the settings-panel implementations. It depends on core and the configuration parser. |
| `hyprcube` | The executable: CLI parsing, shared configuration, panel registration, preview/save orchestration, and the Wayland/SCTK event loop. |

The binary is the composition root. `hyprcube-core` and
`hyprcube-hyprconf` do not depend on the panels crate or executable. Shared
third-party dependency versions belong in the workspace manifest unless a
crate has a documented reason to diverge.

## Native Wayland lifecycle and rendering

`hyprcube::wayland` connects through `wayland-client` and Smithay Client
Toolkit (SCTK). It binds compositor, xdg-shell, shared-memory, seat, output,
and registry state; creates an xdg toplevel; allocates a shared-memory buffer
pool; and dispatches the Wayland event queue until the window closes.

Rendering is custom rather than delegated to a general-purpose GUI toolkit:

- `tiny-skia` draws into a pixmap whose pixels are copied to the Wayland
  shared-memory buffer.
- `cosmic-text`, through `hyprcube-core::text`, shapes, measures, and draws
  text.
- Core widgets implement controls such as toggles, sliders, dropdowns, text
  inputs, color pickers, and scroll areas.
- Palette TOML files under `crates/hyprcube-core/themes/` are embedded at compile time with
  `include_str!`; they are not read from an installed theme directory at
  runtime.

The sidebar selects a panel from `PanelRegistry`. `SettingsPanel::fields()`
becomes a `FormLayout`, which selects widgets by field type and routes pointer,
scroll, and keyboard events. A widget reports a
`WidgetAction::ValueChanged`; the application applies the value to the active
panel and asks the preview layer to process the corresponding Hyprland field.

`SettingsPanel` also exposes custom paint and event hooks. The main loop routes
custom-layout panels through those hooks and uses `FormLayout` for ordinary
field-driven panels. Appearance, Display, Hyprpaper, History, and About use
custom layouts; the remaining configuration panels use generated forms.

`PanelRegistry` creates all panels and filters optional HyprDeck, HyprSaver,
and Hyprpaper panels according to application or configuration availability.
It shares one
`HyprlandConfig` between the Hyprland and Input panels and one `Palette`
between the Appearance panel and renderer. Both use `Arc<Mutex<_>>` so those
panels operate on common in-memory values.

## Configuration and preview flow

```text
hyprland.conf ──parse──> shared HyprlandConfig ──edit──> PreviewEngine
                                                │             │
palette TOML ─────load──> shared Palette ──────┘             └─ Hyprland IPC
                                                │
                                      Save ─────┴──> files on disk + reload
                                      Revert ─────> undo IPC + reload files
```

At startup, `PanelRegistry` loads the structure-preserving Hyprland parser and
palette into `SharedConfigs`. Hyprland and Input changes update their shared
`HyprlandConfig`; Appearance changes update the shared `Palette`. HyprDeck is
a panel-local TOML editor backed by HyprDeck's versioned configuration contract:
it invokes `hyprdeck --print-config-schema` and builds the theme, notification,
and module controls from that schema. HyprSaver and Hyprpaper own panel-local
configuration editors. HyprSaver can launch its native preview process and
apply a snapshot-friendly override collection to native HyprSaver keys; its
optional wallpaper override is represented as a later Hyprpaper source.

When a field has a Hyprland section, `PreviewEngine` sends a `keyword` command
over the Hyprland command socket and records the old value in its undo stack.
Palette and other fields without a Hyprland section do not have an IPC keyword
and do not mark that stack dirty.

Save writes the shared `hyprland.conf` and palette plus each dirty panel-local
configuration. HyprDeck is validated with `hyprdeck --validate-config` before
an atomic replacement with a `.bak` copy; HyprSaver and Hyprpaper use atomic
same-directory replacements. A save error aborts the remaining flow: Hyprland
is not reloaded and the preview undo stack is retained. Revert
replays recorded IPC values in reverse, clears that stack, and reloads shared
and panel-local configuration from disk. HyprDeck requires a restart after a
successful save because it has no runtime reload protocol. Exact paths, failure
behavior, and side effects are documented in [Runtime safety](runtime-safety.md).

The Display implementation is distinct. It queries monitors through direct IPC
with a `hyprctl -j monitors` fallback and sends synchronous `keyword monitor`
requests for changed settings. Hyprpaper's explicit Apply Live button invokes
the current `hyprctl hyprpaper wallpaper` interface. Those requests bypass
`PreviewEngine` and its undo stack.

## Current limitations

- `--daemon` is recognized but prints that daemon mode is not implemented and
  exits.
- Wayland does not provide a portable way to embed HyprSaver's independent
  `xdg_toplevel` inside HyprCube. The preview control therefore opens
  `hyprsaver --preview` as a separate native window.
- `AppConfig` serializes palette and live-preview fields, but current runtime
  behavior does not consult them.

Runtime behavior should be inferred from current source and verified only in
an explicitly authorized isolated session. This document intentionally does
not maintain a speculative roadmap.

For document ownership and navigation, see the [documentation index](README.md).
