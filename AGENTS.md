# HyprCube contributor guide

This file is the repository-wide operating contract for people and automated
agents. Read it before making changes. It is provider-neutral and applies
unless a more-local `AGENTS.md` adds scoped instructions.

## Start here

- Read the root [README](README.md) for the project entry point.
- Use [docs/README.md](docs/README.md) to find the authoritative topic guide.
- Before editing, inspect the affected code and `git status --short`.
- Keep each change focused. Preserve existing user changes; do not revert,
  reformat, rename, or clean up unrelated work.

## Repository map

The Cargo workspace is defined in [Cargo.toml](Cargo.toml) and contains four
crates:

| Crate | Responsibility | Dependency direction |
| --- | --- | --- |
| `hyprcube-core` | rendering primitives, widgets, palette, IPC, undo | leaf utility crate |
| `hyprcube-hyprconf` | structure-preserving Hyprland configuration parsing and writing | leaf parser crate |
| `hyprcube-panels` | settings-panel models, forms, and panel rendering | depends on `core` and `hyprconf` |
| `hyprcube` | application binary, Wayland loop, CLI, registry, preview/save orchestration | depends on all workspace libraries |

Keep dependency flow toward the binary; do not make a lower-level crate depend
on a panel or the application. Declare shared third-party dependency versions
in the root `[workspace.dependencies]` and consume them from member crates with
`{ workspace = true }` unless a documented exception is necessary.

See [docs/architecture.md](docs/architecture.md) before changing boundaries,
state ownership, rendering, or configuration behavior.

## Working rules

- Make the smallest complete change that satisfies the task. Do not invent a
  broad refactor from an isolated issue.
- Treat the working tree as shared: identify pre-existing failures and report
  them, but do not fix unrelated failures without authorization.
- Preserve configuration fidelity. Configuration paths, persistence, preview,
  save, and revert behavior are safety-sensitive; follow
  [docs/runtime-safety.md](docs/runtime-safety.md).
- Keep public behavior, CLI help, and user-facing documentation synchronized
  when a change affects them.
- Update documentation when changing crate boundaries, supported workflows,
  configuration paths or format, IPC/live-preview behavior, safety guarantees,
  or a durable architectural decision. Use a decision record for the last
  category.
- Do not add provider-specific instruction files or duplicate this guidance.
  Add a nested `AGENTS.md` only under the threshold in
  [docs/README.md](docs/README.md).

## Validation and external state

Follow the proportionate validation ladder in
[docs/development.md](docs/development.md). Run the narrowest relevant checks
first, then broaden only as the change warrants.

Do not casually launch the GUI, connect to a live Hyprland IPC session, save or
reload configuration, reset configuration, run `install.sh`, or invoke `sudo`.
Those actions affect external or user state. Perform them only with explicit
authorization for the exact action and an isolated, safe runtime setup. Report
what ran and any pre-existing or environment-caused failures.
