# 0001: HyprDeck CLI configuration contract

Status: Accepted

## Context

HyprCube previously duplicated a small, obsolete set of HyprDeck fields and
saved none of its edits. Duplicating module schemas would drift as HyprDeck
adds or changes configuration.

## Decision

HyprCube obtains the versioned HyprDeck schema through
`hyprdeck --print-config-schema` and validates candidate configurations through
`hyprdeck --validate-config`. It stores edited values as typed TOML and writes
the configuration atomically with a backup. The panel supports only the
declared contract version and requires a HyprDeck restart after saving.

## Consequences

HyprCube does not depend on HyprDeck's Wayland or renderer crates, avoiding a
cross-repository dependency and TOML-version conflict. The HyprDeck executable
must be available on `PATH`, or supplied with `HYPRDECK_BIN`, for its schema and
validation commands. A schema failure leaves the installed configuration
unchanged and explains the problem in the panel.

## Alternatives considered

Directly depending on HyprDeck's runtime crates would couple two applications
to their rendering stacks. Maintaining a local copy of its schema would make
the original drift problem recur. A hand-written list of only common fields
would omit module configuration.
