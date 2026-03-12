# Statum Opportunity Signals

A strong Statum candidate is usually a noun with a stable phase vocabulary and
operations that should change by phase.

## Strong Signals

- a finite lifecycle already exists in code or product language, such as
  `Draft`, `InReview`, `Published`, `Queued`, `Running`, or `Failed`
- repeated status checks across handlers, jobs, or services:
  `match status`, `if phase == ...`, `cannot publish while ...`
- boolean combinations that are acting like hidden states:
  `approved`, `published`, `archived`, `active`, `locked`
- methods that only make sense in one phase, such as `submit`, `approve`,
  `publish`, `activate`, `retry`, `rollback`, or `close`
- data that is only valid in one phase, such as review metadata, lease info,
  failure details, or publish timestamps
- state rebuild from rows, snapshots, or event logs
- service-shaped workflows such as reviews, deployments, payments, orders,
  jobs, protocol sessions, and approval pipelines
- builders or setup flows where order matters and the API should hide illegal
  calls

## Medium Signals

- a clean status enum already exists, but enforcement is still runtime-only
- tests spend noticeable effort proving that illegal call order is rejected
- transition logic is duplicated across API handlers, database code, and worker
  loops
- the team already explains the feature in stable lifecycle language

## Weak or Negative Signals

- the workflow is user-authored, graph-shaped, or plugin-defined
- states are still being renamed or reordered constantly
- the status is mainly for reporting, filtering, or analytics
- most branching is runtime business policy inside one phase
- the code needs a small invariant check, not a protocol model
- the domain is mostly ad hoc UI state or ephemeral request composition

## Quick Triage

Ask these questions before recommending Statum:

1. Are there finite named phases?
2. Are illegal transitions expensive or noisy in production?
3. Should available methods change by phase?
4. Does some data only exist in specific phases?
5. Is the lifecycle stable enough to codify now?
6. Is there a rebuild path from rows or event streams?

Three or more strong "yes" answers usually justify a deeper pass. One or two
answers usually mean "keep runtime validation for now."

## Map the Candidate to Statum

- staged entity -> `#[machine]`
- lifecycle phases -> `#[state]`
- durable shared context -> machine fields
- phase-only payloads -> state data
- legal edges -> `#[transition]` impl blocks
- row rebuilds -> `#[validators]`
- append-only event logs -> `statum::projection` first, then `#[validators]`

## Evidence Agents Should Cite

When an agent recommends Statum, it should point to:

- the current files and symbols that encode the lifecycle
- duplicated guard logic or invalid transition checks
- the methods that should disappear outside a specific phase
- the current state-specific data or optional fields
- the persistence boundary, if rebuild or rehydration is part of the design
- the likely migration risk: low, medium, or high

## Good Anchors in This Repo

- review flow: [../../statum-examples/src/toy_demos/13-review-flow.rs](../../statum-examples/src/toy_demos/13-review-flow.rs)
- rows to typed machines:
  [../persistence-and-validators.md](../persistence-and-validators.md)
- event-log rebuild:
  [../../statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
- protocol/session lifecycle:
  [../../statum-examples/src/showcases/tokio_websocket_session.rs](../../statum-examples/src/showcases/tokio_websocket_session.rs)
