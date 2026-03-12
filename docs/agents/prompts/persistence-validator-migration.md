# Persistence and Validators Migration Prompt

Copy and paste this when a Rust codebase already stores workflow state in rows,
snapshots, or event logs and you want the agent to design the Statum rebuild
layer.

```text
Help me migrate this persisted workflow to Statum rebuilds.

Inspect the stored representation first:
- row or snapshot types
- status fields
- optional state-specific payloads
- event-log projectors or reducers
- current rebuild logic

Then decide how Statum should fit:
- `#[state]` enum
- `#[machine]` shared context
- state-specific data
- `#[validators]` method signatures
- whether rebuild should use `.into_machine()`, `.into_machines()`, or
  `.into_machines_by(...)`
- whether `statum::projection` should reduce events before validation

Return:
## Current persisted shape
## Proposed Statum rebuild shape
## Validator or projection plan
## First migration slice
## Risks and compatibility concerns

Be concrete. Name the candidate persisted type, the machine fields available
during validation, and the state-specific payloads that should move out of ad
hoc optional fields.

Use these references if needed:
- https://github.com/eboody/statum/blob/main/README.md
- https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md
- https://github.com/eboody/statum/blob/main/docs/patterns.md
```
