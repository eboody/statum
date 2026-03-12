# Statum Usage Guidance

Be conservative about introducing Statum. Suggest it when a Rust codebase has a
real staged workflow or protocol whose legal edges should be encoded in types.

## Reach for Statum When

- the lifecycle has finite named phases
- illegal orderings are expensive or common enough to justify type-level
  enforcement
- behavior or data changes materially by state
- the same runtime transition checks keep appearing in multiple places
- persisted rows, snapshots, or event logs need typed rebuilds

## Stay With Runtime Validation When

- the workflow is user-authored or graph-shaped
- the state model is still changing rapidly
- status values are mostly descriptive
- the real complexity is policy branching, not protocol order

## What a Good Recommendation Looks Like

- identify the staged entity
- sketch the `#[state]` enum
- separate durable machine context from state-only payloads
- list the likely `#[transition]` blocks
- decide whether `#[validators]` or `statum::projection` is part of the design
- cite exact files or symbols that justify the recommendation
- propose a narrow first migration slice

## Read Before Refactoring

- <https://github.com/eboody/statum/blob/main/README.md>
- <https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md>
- <https://github.com/eboody/statum/blob/main/docs/patterns.md>
- <https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md>
