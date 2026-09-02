# Architecture decisions

This directory records short Architecture Decision Records (ADRs) for durable,
cross-cutting choices. It is not a task log, implementation diary, or a place
to restate code details already covered by [the architecture guide](../architecture.md).

## When to add an ADR

Add one when a decision has a lasting effect across crates, runtime behavior,
external compatibility, security/safety posture, or maintenance strategy. Good
examples include changing the rendering model, configuration authority, or the
crate dependency direction.

Do not add an ADR for routine refactors, feature status, temporary workarounds,
or speculative ideas. Do not retroactively invent records for existing choices
unless a new change needs that historical context.

## File and status convention

Use a zero-padded, monotonically increasing filename:

```text
NNNN-short-kebab-case-title.md
```

For example, `0001-example-decision.md`. Start at `0001` when the first actual
decision is recorded. Use one of these statuses near the top of each record:

- `Proposed`
- `Accepted`
- `Superseded by NNNN-title.md` (link the filename in the actual record)
- `Deprecated`

Keep superseded records in place; link the newer record instead of rewriting
history.

## Template

```md
# NNNN: Short decision title

Status: Proposed

## Context

What durable problem or constraint requires a decision?

## Decision

What was chosen, including the scope of the choice?

## Consequences

What becomes easier, harder, required, or intentionally unsupported?

## Alternatives considered

What viable alternatives were considered, and why were they not chosen?
```

Link a new ADR from the [documentation index](../README.md) when it becomes a
useful entry point for contributors. Keep user-facing setup and runtime effects
in their canonical guides, especially [Runtime safety](../runtime-safety.md).
