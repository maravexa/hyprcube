# HyprCube documentation

This directory contains durable project knowledge that is too detailed for the
root README or repository-wide contributor guide.

## Index

- [Project README](../README.md) — product overview, build, and user-facing
  quick start.
- [Contributor and agent guide](../AGENTS.md) — repository-wide working
  contract and safety rules.
- [Architecture](architecture.md) — crate boundaries, dependency direction,
  and system design.
- [Development](development.md) — prerequisites and the validation ladder.
- [Runtime safety](runtime-safety.md) — configuration, IPC, and external-state
  safeguards.
- [Release guide](release.md) — release artifacts, crates.io publication order,
  and AUR preparation.
- [Decisions](decisions/README.md) — decision-record policy and accepted
  cross-cutting choices, including the [HyprDeck CLI contract](decisions/0001-hyprdeck-cli-contract.md).

## Authority and freshness

Source code and Cargo manifests are authoritative for current behavior,
dependencies, commands, and supported interfaces. Documentation explains that
behavior and must link to, rather than duplicate, volatile implementation
inventories. `AGENTS.md` is authoritative for repository-wide working rules;
a more-local `AGENTS.md` may add instructions only for its directory subtree.
Decision records document the rationale for accepted, cross-cutting choices;
they do not override code or current safety requirements.

Update the relevant document in the same change when modifying a documented
workflow, crate boundary, runtime/configuration behavior, IPC contract, safety
guarantee, or architectural decision. Keep status statements qualitative and
dated only when necessary; do not maintain static roadmaps or test counts.
Remove or revise documentation that no longer matches the source instead of
leaving historical claims as current guidance.

## Scoped guidance

The root `AGENTS.md` applies everywhere. Add a nested `AGENTS.md` only when a
directory has enduring, materially different instructions—such as a separate
build/test workflow, generated-file boundary, security constraint, or external
integration procedure—that cannot be expressed once at the root. Keep it
short, link back to root guidance, and do not use it to repeat general style or
tool preferences.
