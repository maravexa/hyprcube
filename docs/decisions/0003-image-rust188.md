# 0003: Image 0.25.10 and Rust 1.88 minimum

Status: Accepted

## Context

The prior image 0.25.9 upper bound preserved Rust 1.86 compatibility but blocked
image 0.25.10, whose declared minimum compiler is Rust 1.88.

## Decision

Raise the workspace minimum to Rust 1.88 and use image 0.25.10. Keep the
minimum-compiler check and workspace regressions aligned with that requirement.
Retain the existing image format features and avoid unrelated dependency updates.
This supersedes the Rust 1.86 compatibility floor in
[0002](0002-smithay-rust186.md); its Smithay runtime validation limits remain.

## Consequences

Source builds need Rust 1.88 or newer. Prebuilt binaries do not require Rust.
Both the minimum compiler and a current development compiler must pass the
locked workspace tests. This dependency change does not establish compositor
or visual acceptance.

## Alternatives considered

Keeping image 0.25.9 would retain the older compiler floor but leave the image
update blocked. The approved backlog includes the compiler minimum change.
