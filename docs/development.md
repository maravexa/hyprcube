# Development guide

## Setup

Use Rust on Linux. The workspace declares Rust 1.85 and CI retains a dedicated MSRV check. The `image` dependency is bounded to 0.25.9 because 0.25.10 requires Rust 1.88. Keep the locked dependency set compatible with the declared MSRV. Use a current stable toolchain compatible with the locked dependency set for development. CI enforces workspace formatting, check, unit and doc tests, strict Clippy, Markdown linting, a release build, and CLI help/version smoke checks. Markdown lint intentionally permits long prose and table rows; local verification remains important for focused changes.

The GUI is a Wayland application. A real GUI run needs a Wayland session, and Hyprland-specific behavior additionally needs a live Hyprland IPC environment. Do not treat GUI startup as a general development check; read [runtime safety](runtime-safety.md) before choosing manual runtime testing.

Cargo’s lockfile is tracked. Use `--locked` for commands that resolve dependencies so an ordinary check does not update it.

## Verification

Choose the smallest relevant command first, then run the workspace checks appropriate to a Rust change.

| Change | Focused verification | Broader verification before handoff |
| --- | --- | --- |
| `hyprcube-core` | `cargo test -p hyprcube-core --locked` | `cargo check --workspace --all-targets --locked` and `cargo test --workspace --locked` |
| `hyprcube-hyprconf` | `cargo test -p hyprcube-hyprconf --locked` | `cargo check --workspace --all-targets --locked` and `cargo test --workspace --locked` |
| `hyprcube-panels` | `cargo test -p hyprcube-panels --locked` | `cargo check --workspace --all-targets --locked` and `cargo test --workspace --locked` |
| Binary/application logic | `cargo check -p hyprcube --all-targets --locked` | `cargo check --workspace --all-targets --locked` and `cargo test --workspace --locked` |
| Rust code in any crate | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` | Include the focused and workspace checks above. |
| Documentation only | Review changed links and commands against the source. | Cargo checks are not required unless the documentation changes a generated or code-adjacent artifact. |

`cargo run --locked -p hyprcube -- --help` is a safe CLI smoke check: argument parsing exits before the panel registry and Wayland event loop start. Do not substitute `cargo run --locked -p hyprcube` for it. That starts the GUI and can touch user configuration; do not run the installer, reset configuration, or exercise live IPC unless the task explicitly requires it.

Run formatting and strict clippy checks for Rust changes. If an existing failure is outside the requested change, do not perform unrelated cleanup just to make the command pass: report the command, its result, and why it is unrelated. Format and correct code you touch.

## Tests and code conventions

Unit tests live beside their implementation in `#[cfg(test)] mod tests`; add coverage near the behavior being changed. Parser tests should preserve the contract that comments, blank lines, ordering, and nested configuration structure survive a parse/serialize round trip. Widget and panel tests should cover value handling and bounds/interaction behavior without requiring a real Wayland or Hyprland session.

Prefer workspace dependency versions from the root `Cargo.toml`. Follow nearby error and observability conventions: define typed errors with `thiserror` and use `tracing` rather than ad-hoc terminal output for application diagnostics. Configuration-path resolution currently has legacy duplication in the parser and binary; do not expand it. Prefer the core helper where crate dependencies allow, and update [runtime safety](runtime-safety.md) if path or fallback behavior changes. See [architecture](architecture.md) for the crate boundaries.

## Keeping documentation current

Update the relevant durable document when a crate boundary, configuration write path, IPC behavior, validation command, or runtime safety property changes. Record consequential architectural choices in the [decision log](decisions/README.md). The root [AGENTS.md](../AGENTS.md) defines the repository-wide working rules.

## Dependency security checks

The read-only security workflow runs on pull requests, main pushes, a weekly
schedule, and manual dispatch. It runs pinned `cargo-audit` against `Cargo.lock`
and preserves its exit status: vulnerabilities and scan errors fail the job.
Maintenance notices remain visible in the log and the `dependency-audit`
artifact without becoming automatic version upgrades or ignored advisories.
The artifact is retained for 14 days, including when scanning fails. No issue
or check-report API write permission is needed.

CI limits Cargo to two build jobs and keeps lint independent of compilation.
Workflow definitions cannot enable repository Actions settings or make checks
required in branch protection; those settings need a separate maintainer review
if pull requests do not receive runs.

Smithay 0.20 uses xkbcommon 0.8, allowing the lockfile to use patched
`memmap2 0.9.11` throughout. Keep `calloop` locked to 0.14.4 for Rust 1.85;
0.14.5 requires Rust 1.86. Dependency refreshes must pass the MSRV job.

The Smithay 0.20 keyboard handler accepts raw modifiers and forwards
compositor key-repeat events through the existing key-press handler. Test
keyboard repeat and modifier handling in an authorized isolated Wayland session
before accepting this migration; compilation does not validate compositor behavior.
