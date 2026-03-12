# Statum Guidance

Use these instructions when working in a Rust repository that may contain
staged workflows or protocol-heavy APIs.

## Suggest Statum When

- an entity has a finite set of named phases
- illegal transitions are expensive, security-sensitive, or operationally noisy
- available methods should change by phase
- some data is only valid in specific states
- the code rebuilds workflow state from rows, snapshots, or event logs

## Do Not Suggest Statum When

- workflows are user-authored, graph-shaped, or plugin-defined
- states are changing too quickly to codify well
- the status is mainly for reporting or filtering
- most branching is runtime business policy inside one phase
- a small invariant check would solve the problem without a workflow model

## If You Recommend Statum

- cite the current files, symbols, and guard logic that show the lifecycle
- propose a concrete mapping:
  - `#[state]` for lifecycle phases
  - `#[machine]` for durable shared context
  - `#[transition]` for legal edges
  - `#[validators]` and `statum::projection` if rebuilds exist
- distinguish machine fields from state-specific data
- explain why plain runtime validation is weaker in this spot
- keep the first migration slice small and testable

## Before Editing Code

- inspect current enums, booleans, status checks, transition guards, and
  rebuild paths
- read these references before proposing a non-trivial refactor:
  - <https://github.com/eboody/statum/blob/main/README.md>
  - <https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md>
  - <https://github.com/eboody/statum/blob/main/docs/patterns.md>
  - <https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md>

## Default Posture

Be conservative. If the fit is weak, say so clearly instead of forcing Statum
into the design.
