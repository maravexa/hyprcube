# 0002: Rust 1.86 minimum for Smithay 0.21

Status: Accepted

## Context

Smithay Client Toolkit 0.21.1 requires Rust 1.86 and replaces the separate
protocol delegation macros with `delegate_dispatch2!`. Retaining a declared
Rust 1.85 minimum would make the dependency migration unsupported.

## Decision

Raise the workspace minimum and dedicated CI check together to Rust 1.86.
Use Smithay 0.21.1 and its dispatch API while retaining the existing keyboard,
pointer, window, rendering, and configuration behavior. Keep the TOML 0.8
compatibility policy and cosmic-text version unchanged.

## Consequences

Rust 1.85 is no longer supported for building this workspace. Validate the
exact new minimum independently of current-stable release builds. Wayland
runtime smoke tests remain necessary because compilation cannot validate event
handling. This decision does not authorize other dependency upgrades.

## Alternatives considered

Keeping Smithay 0.20 preserves Rust 1.85 support but defers this migration.
Ignoring the dependency's declared minimum would make the support promise
unreliable and is not an acceptable alternative.
