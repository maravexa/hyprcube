# Runtime safety

HyprCube deliberately interacts with the current user's Hyprland session and
configuration. Treat a GUI launch, a save, a revert, or a display change as an
external-state operation. This page records the current paths and effects so
that manual testing can be planned safely.

## Paths and discovery

The intended configuration root is `$XDG_CONFIG_HOME/hypr`. If
`XDG_CONFIG_HOME` is unset, normal operation uses `$HOME/.config/hypr`.
Fallback resolution is not fully centralized: the core helper and binary save
path fall back to `/tmp/.config/hypr` when `HOME` is also unset, while
`HyprlandConfig::load_default` requires `HOME` and will panic without it. Set
`XDG_CONFIG_HOME` explicitly in isolated tests rather than relying on a
fallback.

| File | Role | How it changes |
| --- | --- | --- |
| `$XDG_CONFIG_HOME/hypr/hyprcube.toml` | Serialized application fields for window dimensions, selected panel, palette name, and live-preview preference. | Saved when the normal application exits; deleted by `hyprcube --reset-config`. Only window dimensions and selected panel currently affect runtime behavior. |
| `$XDG_CONFIG_HOME/hypr/hyprcube-palette.toml` | Shared HyprCube palette. | Created with defaults when startup cannot load it; overwritten by Save. |
| `$XDG_CONFIG_HOME/hypr/hyprland.conf` | Hyprland configuration parsed by `hyprcube-hyprconf`. | Written by Save from shared in-memory config. |
| `$XDG_CONFIG_HOME/hypr/hyprland.conf.bak` | Fixed backup name for the previous Hyprland configuration. | Replaced from the pre-save `hyprland.conf` when that file exists. |
| `$XDG_CONFIG_HOME/hypr/hyprdeck.toml` | Optional HyprDeck configuration. | Read and edited using HyprDeck's versioned CLI schema. Save validates a same-directory temporary file, then atomically replaces this file. |
| `$XDG_CONFIG_HOME/hypr/hyprdeck.toml.bak` | Previous HyprDeck configuration. | Replaced from the pre-save `hyprdeck.toml` immediately before a successful replacement. |
| `$XDG_CONFIG_HOME/hypr/hyprsaver.toml` | Optional HyprSaver configuration, including the snapshot-friendly `[hyprcube_override]` collection. | Atomically replaced when the panel is dirty; enabled override palette and playlist values are copied to native `[general]` keys. |
| `$XDG_CONFIG_HOME/hypr/hyprpaper.conf` | User Hyprpaper root configuration. | Existing text is preserved while HyprCube appends or removes its managed `source` lines as required. |
| `$XDG_CONFIG_HOME/hypr/hyprpaper-hyprcube.conf` | Per-monitor assignments managed by the Hyprpaper panel. | Atomically replaced on Save. |
| `$XDG_CONFIG_HOME/hypr/hyprpaper-hyprcube-theme.conf` | Optional wallpaper override belonging to the HyprSaver/HyprCube theme collection. | Atomically replaced or marked disabled when the override changes. |
| `$XDG_STATE_HOME/hyprcube/config-history/` | Private Git repository used by the History panel. | Mirrors only the managed configuration files and records commits around saves and restores. Falls back to `$HOME/.local/state`; if neither variable is set, uses `/tmp`. |

The Hyprland command socket is discovered from
`$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`. If either
environment variable is missing or that socket is absent, the main preview
engine continues without IPC. The Display panel independently uses the same
discovery rule for monitor queries and monitor updates.

## Startup and normal edits

Starting the normal application is not read-only. During registry creation, if
the palette cannot be loaded, the application attempts to create the default
`hyprcube-palette.toml` at the path above. On a normal exit it also attempts to
save `hyprcube.toml`.

For Hyprland-backed fields, a change first updates the panel's shared in-memory
configuration. `PreviewEngine` then sends `keyword <section:key> <value>` to
the Hyprland command socket when connected and records the prior value in an
undo stack. IPC errors are logged; they do not prevent the in-memory change.
Fields without a Hyprland section, including palette fields, have no preview
IPC command.

Display edits follow a separate path. The Display custom panel sends
synchronous `keyword monitor ...` requests
directly to the command socket for monitor enablement, resolution, refresh
rate, scale, transform, and position drags. These requests do not enter
`PreviewEngine`'s undo stack. Display edits do not add monitor lines to the
shared `HyprlandConfig`.

Hyprpaper changes remain in panel memory until Save, except for the explicit
**Apply Live** action, which invokes `hyprctl hyprpaper wallpaper` immediately.
Saving an enabled HyprSaver theme wallpaper override also attempts the same
live operation for each connected monitor. The HyprSaver **Open Preview**
action starts a separate `hyprsaver --preview --config ...` process.

## Save, reload, and revert

Selecting **Save** performs these operations:

1. If `$XDG_CONFIG_HOME/hypr/hyprland.conf` exists, copy it to the fixed path
   `$XDG_CONFIG_HOME/hypr/hyprland.conf.bak`.
2. Write the current shared parser output to
   `$XDG_CONFIG_HOME/hypr/hyprland.conf`.
3. Write the current shared palette to
   `$XDG_CONFIG_HOME/hypr/hyprcube-palette.toml`.
4. When the main IPC client is connected, send `reload` to Hyprland and query
   `configerrors` for feedback.
5. Save dirty panel-local HyprDeck, HyprSaver, and Hyprpaper configuration.
6. Capture the resulting managed files in private configuration history.
7. Clear the normal preview undo stack.

The shared file writes are sequential rather than transactional, so a later
shared-file failure can leave an earlier write in place. A HyprDeck save itself
is validated before replacement and atomic within its destination directory.
Any save error is surfaced in the application, does not reload Hyprland, and
does not commit the normal preview undo stack. Each successful pre-save copy
replaces the fixed-name backup.

Selecting **Revert** replays normal preview entries in reverse order through
the main IPC client, clears that stack, then reloads `hyprland.conf` and
`hyprcube-palette.toml` from disk into shared state. It does not restore
Display-panel changes made through direct monitor IPC.

The Save/Revert action bar is driven by both the normal preview undo stack and
panel-local dirty state. HyprDeck, HyprSaver, and Hyprpaper changes therefore
participate even though they have no normal Hyprland IPC keyword. Restart
HyprDeck after a successful save to load its new configuration.

`hyprcube --reset-config` deletes only
`$XDG_CONFIG_HOME/hypr/hyprcube.toml` and exits. It does not delete the palette
or Hyprland configuration, but it is still a destructive command and requires
explicit authorization.

## Installer

`install.sh` builds a release binary and then runs:

```text
sudo install -Dm755 target/release/hyprcube /usr/local/bin/hyprcube
```

It therefore requests elevated privileges and changes `/usr/local/bin`. Do
not run it without explicit authorization.

## Configuration history

The History panel shells out to the installed `git` executable and uses only a
private repository under the state directory. It does not modify the Git
repository containing HyprCube or publish configuration anywhere. Named
snapshots are private lightweight tags under `refs/tags/snapshot/`.

Selecting Restore first captures the live managed files, reconstructs the
selected revision into same-directory temporary files, and renames them into
place. If one replacement fails, HyprCube attempts to restore the complete
pre-operation file set. A successful restore is recorded as a new commit, so
the previous state remains selectable for fast-forward-style recovery.

History actions are blocked while the editor has unsaved changes. Restoring
reloads Hyprland and the in-memory panel state; HyprDeck and HyprSaver must be
restarted when their restored settings need to take effect.

## Authorized isolated GUI testing

Run GUI/manual tests only when they are explicitly authorized and all of the
following are true:

- `XDG_CONFIG_HOME` points to a new temporary directory owned by the test, not
  the user's normal configuration directory.
- The process cannot access a live Hyprland command socket: do not inherit a
  usable pair of `XDG_RUNTIME_DIR` and `HYPRLAND_INSTANCE_SIGNATURE`, and do
  not provide the corresponding socket by another route.
- The Wayland environment itself is an approved disposable test compositor or
  session, and the tester has reviewed the startup and exit writes described
  above.
- The test does not use `--reset-config`, Save, the installer, or any command
  requiring elevated privileges unless that exact operation is separately
  authorized.

For the code-level data flow, see [Architecture](architecture.md). For the
repository-wide operational rules, see the root `AGENTS.md` and the
[documentation index](README.md).
