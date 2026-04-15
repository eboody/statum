# Statum Opportunity Signals

A strong Statum candidate is not just "has steps." It is a value whose phase
should change what methods are legally available on that value.

If you pressed `.` before and after a transition, and you would want to see a
meaningfully different method surface, that is the right starting signal.

Start with typestate as the umbrella idea:

- smaller typestate surface or builder when validation, resolution, or
  construction order should hide illegal calls
- workflow machine when the value itself moves through durable runtime phases

## Strong Signals

- the same struct or value goes through named phases and different phases
  should expose different methods
- callers could misuse methods by calling them in the wrong order
- intermediate states escape a function or module and are interacted with
  directly
- the code is already simulating legal next steps with stage structs, enums,
  helper names, or comments
- phase-specific data is awkwardly modeled with `Option`, booleans, or
  defensive checks because it is only valid in some states
- branching legal paths would read better as distinct typed transitions or
  wrappers
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
- one workflow owns another workflow with its own stable phases
- governance or approval status changes what operations are legal elsewhere
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

- the code is plain linear orchestration, and the intermediate values are
  private locals rather than a real API surface
- a wrapper would mostly narrate a story without removing meaningful illegal
  calls from `.` completion
- the workflow is user-authored, graph-shaped, or plugin-defined
- states are still being renamed or reordered constantly
- the status is mainly for reporting, filtering, or analytics
- most branching is runtime business policy inside one phase
- the code needs a small invariant check, not a protocol model
- the domain is mostly ad hoc UI state or ephemeral request composition
- the interesting part is uncontrolled policy search, not stable protocol order

## Quick Triage

Ask these questions before recommending Statum:

1. Should the value's legal method surface change by phase?
2. Would callers misuse methods by calling them in the wrong order today?
3. Do intermediate states escape and get interacted with directly?
4. Does some data only exist in specific phases?
5. Are there finite named phases?
6. Is the lifecycle stable enough to codify now?
7. Is there a rebuild path from rows or event streams?
8. Does a parent workflow own a child lifecycle worth modeling separately?

Four or more strong "yes" answers usually justify a deeper pass. One or two
answers usually mean "keep runtime validation or plain local staging for now."

## Map the Candidate to Statum

- if the value lives through durable runtime phases -> `#[machine]` +
  `#[state]` + `#[transition]`
- if the pressure is staged reconstruction from persisted facts ->
  `#[validators]`
- if append-only events are the source of truth -> `statum::projection` first,
  then `#[validators]`
- if the only real problem is staged construction with private locals -> leave
  it alone or use a smaller typestate surface before reaching for a full
  machine

## Evidence Agents Should Cite

When an agent recommends Statum, it should point to:

- the current files and symbols that encode the lifecycle
- why the method surface should differ by phase
- duplicated guard logic or invalid transition checks
- the methods that should disappear outside a specific phase
- the current state-specific data or optional fields
- the owned child workflow or approval flow that should not be flattened
- the persistence boundary, if rebuild or rehydration is part of the design
- what should stay runtime policy instead of becoming typestate
- the likely migration risk: low, medium, or high

## Good Anchors in This Repo

- review flow: [../../statum-examples/src/toy_demos/13-review-flow.rs](../../statum-examples/src/toy_demos/13-review-flow.rs)
- nested workflow:
  [../../statum-examples/src/toy_demos/11-hierarchical-machines.rs](../../statum-examples/src/toy_demos/11-hierarchical-machines.rs)
- rows to typed machines:
  [../persistence-and-validators.md](../persistence-and-validators.md)
- event-log rebuild:
  [../../statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
- protocol/session lifecycle:
  [../../statum-examples/src/showcases/tokio_websocket_session.rs](../../statum-examples/src/showcases/tokio_websocket_session.rs)
