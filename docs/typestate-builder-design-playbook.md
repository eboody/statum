# Typestate Builder Design Playbook (Rust + Statum)

If there are particular stages that an abstract entity goes through, and there is meaningful ordering between those stages, you should strongly consider typestate.

That sentence is the center of this guide.

Typestate is not only a way to model states. It is a way to encode protocol rules directly into your API so invalid flows are not representable. In Rust, that can remove entire bug classes before tests run.

This playbook is opinionated:

- Default to typestate for stable, protocol-heavy workflows.
- Keep runtime validation for highly dynamic edges.
- Be explicit about boundaries so complexity stays proportional.

The quality bar for this approach is not only correctness. A good typestate design should also improve:

- readability (state names and method availability explain behavior),
- modularity (state-specific logic lives in state-specific impl blocks),
- extensibility (new stable states/edges can be added without rewiring everything),
- expressiveness (the API communicates lifecycle intent directly),
- idiomaticity (Rust ownership + type system are used naturally, not fought),
- correctness (illegal protocol edges are unrepresentable).

## What This Guide Helps You Decide

Use this guide when you are asking:

- "Should this domain be a typestate machine?"
- "How do I design the states cleanly before writing methods?"
- "How do I map the design into Statum macros without fighting the model?"

The workflow below is intentionally practical. You can run it on a whiteboard first, then implement.

## Canonical Running Example

We will use a document publication flow:

- `Draft`
- `InReview(ReviewData)`
- `Published(PublishMeta)`

The exact domain is less important than the structure:

- finite phases,
- clear legal transitions,
- state-specific behavior and data.

## Step 1: Identify the Staged Entity

### What to do

Name the thing that changes phase over time. Use a noun, not a verb.

Good examples:

- `Document`
- `Payment`
- `Job`
- `Deployment`

Then write the sequence as plain language first:

- "A document starts in draft."
- "Draft can be submitted for review."
- "Only reviewed documents can be published."

### Why it matters

If you cannot describe the lifecycle in plain language, you are not ready to encode it in types. Typestate mirrors conceptual protocol, not accidental implementation details.

This is primarily a readability and expressiveness checkpoint. If humans cannot explain the lifecycle simply, the type system should not be asked to encode it yet.

### Common mistake

Starting with methods (`publish`, `approve`, `retry`) before defining lifecycle phases. That usually produces leaky APIs where invalid method calls are still possible.

### Quick candidate pressure-test

Before moving on, force clear yes/no answers:

1. Does this entity have a finite set of phases, not an unbounded graph?
2. Is transition legality mostly protocol-driven, not user-authored?
3. Would an illegal transition be expensive (money, trust, compliance, or recovery time)?

If you answer "no" to two or more, this may be a runtime-validation domain instead of a full typestate domain.

## Step 2: Enumerate States Before Methods

### What to do

Write the finite state set before writing any transition code.

For Statum, that means a `#[state]` enum:

```rust
use statum::state;

#[state]
pub enum DocumentState {
    Draft,
    InReview(ReviewData),
    Published(PublishMeta),
}

pub struct ReviewData {
    pub reviewer: String,
}

pub struct PublishMeta {
    pub published_at_unix: i64,
}
```

Rules of thumb:

- State names should represent business phases, not transport events.
- Use data-bearing variants only when data is truly phase-specific.
- Keep state count minimal but complete.

State vs Event vs Action (keep these separate):

- State: durable phase (`Draft`, `InReview`, `Published`).
- Event: signal that can trigger logic (`ApproveClicked`, `TimerExpired`).
- Action: operation that may cause transition (`submit_for_review`, `publish`).

Keeping these distinct prevents state explosion and keeps APIs legible.

### Why it matters

The enum is your protocol vocabulary. If state names are fuzzy or overloaded, transition logic and error messages degrade quickly.

Well-named states are the biggest readability and idiomaticity win. They make generated types and compiler diagnostics align with domain language.

### Common mistake

Creating transitional pseudo-states like `NeedsValidationAndMaybeApproval`. That bundles decision logic and lifecycle phase into one bucket. Split phases and keep branching in methods.

## Step 3: Define Machine Context (`#[machine]`)

### What to do

Put long-lived context on the machine struct: identifiers, dependencies, shared config.

```rust
use std::sync::Arc;
use statum::machine;

trait Storage {}
trait Publisher {}

#[machine]
pub struct DocumentMachine<DocumentState> {
    id: String,
    storage: Arc<dyn Storage>,
    publisher: Arc<dyn Publisher>,
}
```

Use this split:

- Machine fields: data needed across many states.
- State data: data that only exists or is valid in one state.

Dependency and ownership guidance:

- Put long-lived collaborators (db client, queue handle, repository) on the machine.
- Prefer trait-object handles or generic wrappers that are cheap to move.
- Keep large transient payloads in state data, not on the machine root.
- If a dependency is only needed in one phase, reconsider whether it should be phase data instead.

### Why it matters

Context placement controls API clarity. Good separation keeps state invariants explicit and avoids copying unrelated fields into every variant payload.

This is the main modularity and extensibility lever. A clean split between machine context and state data lets you evolve one without destabilizing the other.

### Common mistake

Putting all data into machine fields "for convenience." You lose one of typestate's biggest wins: state-constrained data guarantees.

## Step 3.5: Respect Statum Macro Boundaries

### What to do

Keep `#[transition]` impl blocks narrowly focused on legal transition methods.

Put non-transition helpers in regular `impl` blocks:

- constructors (`from_command`, `new_with_context`),
- branch helpers (`build`, `route`, `decide` returning enums),
- formatting/inspection helpers.

This avoids mixing protocol edges with orchestration glue.

### Why it matters

Statum macros enforce transition signatures. Helpers that are not transitions can fail macro validation and create noisy APIs.

Separation here improves readability and modularity: transition blocks read like protocol graphs, while regular impl blocks handle setup and policy glue.

### Wrong vs right

```rust
// Wrong: helper method in a #[transition] impl block.
#[transition]
impl PostMessageMachine<Incoming> {
    fn from_command(cmd: PostMessageCommand) -> Self { /* ... */ }
}

// Right: helper in regular impl; transitions stay in #[transition] blocks.
impl PostMessageMachine<Incoming> {
    fn from_command(cmd: PostMessageCommand) -> Self { /* ... */ }
}

#[transition]
impl PostMessageMachine<Incoming> {
    fn validate_message(self) -> Result<PostMessageMachine<Validated>, Error> { /* ... */ }
}
```

### Common mistake

Treating `#[transition]` blocks as general-purpose impl blocks. They are protocol-edge definitions, not generic utility containers.

## Step 4: Encode Legal Transitions (`#[transition]`)

### What to do

Implement transition methods only on legal source states.

```rust
use statum::transition;

#[transition]
impl DocumentMachine<Draft> {
    fn submit_for_review(self, reviewer: String) -> DocumentMachine<InReview> {
        self.transition_with(ReviewData { reviewer })
    }
}

#[transition]
impl DocumentMachine<InReview> {
    fn publish(self, unix_ts: i64) -> DocumentMachine<Published> {
        self.transition_with(PublishMeta { published_at_unix: unix_ts })
    }
}
```

Choose transition helper by target state shape:

- `transition()` for unit target states.
- `transition_with(data)` for data-bearing target states.

Common transition signatures:

```rust
fn approve(self) -> DocumentMachine<Published>;
fn try_publish(self) -> Result<DocumentMachine<Published>, statum::Error>;
fn maybe_publish(self) -> Option<DocumentMachine<Published>>;
```

Use a direct return when transition is always legal from that source state. Use `Result`/`Option` when runtime checks (permissions, feature flags, side-effect outcomes) gate that edge.

### Why it matters

You are expressing legal protocol edges as function signatures. Once encoded, invalid edges stop compiling instead of waiting for runtime checks.

This is where expressiveness and correctness meet: API shape communicates legal workflow, and illegal workflow cannot type-check.

### Common mistake

Adding a broad `impl DocumentMachine<S>` with generic transition methods. That reintroduces invalid paths and defeats typestate constraints.

## Step 4.5: Use a Three-Layer Flow Shape

### What to do

For endpoint/application flows, keep responsibilities explicit:

1. Boundary adapter: parse dynamic input into a typed starting machine.
2. Protocol transitions: concrete-state transition methods encode legal edges.
3. Orchestration: sequence transitions and side effects at the call site.

Example shape:

```rust
let flow = PostMessageMachine::<Incoming>::from_command(cmd);
let flow = flow.validate_message()?;
let flow = flow.apply_moderation(&moderator)?;
let built = flow.build(now);
```

### Why it matters

This pattern makes the happy path easy to read and makes it obvious where runtime uncertainty lives.

It also keeps extensibility high: adding a new stable state usually means adding one transition method and one orchestration step.

### Common mistake

Collapsing all logic into free functions or one large method that hides protocol stages and makes future extensions risky.

## Step 5: Keep Branching and Guards Outside Transition Definitions

### What to do

Branch on runtime conditions in normal methods, then dispatch to explicit transition methods.

```rust
enum ReviewDecision {
    Approve,
    Reject,
}

impl DocumentMachine<InReview> {
    fn decide(self, decision: ReviewDecision) -> Result<DocumentMachine<Published>, statum::Error> {
        match decision {
            ReviewDecision::Approve => Ok(self.publish(now_unix())),
            ReviewDecision::Reject => Err(statum::Error::InvalidState),
        }
    }
}
```

For preconditions, add guard methods:

```rust
impl DocumentMachine<InReview> {
    fn can_publish(&self) -> bool {
        !self.state_data.reviewer.is_empty()
    }
}
```

When runtime branching can lead to multiple target states, return a decision enum that carries typed machines for each branch.

```rust
enum Next {
    Published(DocumentMachine<Published>),
    ReturnedToDraft(DocumentMachine<Draft>),
}
```

### Why it matters

Typestate should encode legal structure. Runtime branching still exists, but it should route into explicit legal edges. This keeps static guarantees and runtime flexibility balanced.

Keeping branching outside transition signatures preserves readability and keeps transition modules focused, which improves modularity.

### Common mistake

Trying to hide all branching inside one giant transition method that returns different next states ad hoc. Model choices explicitly with enums/results.

## Step 5.5: Prefer Associated Methods Over Wrapper Functions

### What to do

Prefer calling methods directly on typed machine states instead of creating top-level forwarding wrappers.

Use top-level functions only when they add boundary adaptation or shared policy.

```rust
// Preferred
let flow = flow.validate_message()?;
let flow = flow.persist(&repo).await?;

// Avoid when it adds no value
let flow = validate_message(flow)?;
let flow = persist(flow, &repo).await?;
```

### Why it matters

Forwarding wrappers create noise without adding invariants. Direct typed calls are usually more readable and expressive.

### Common mistake

Keeping `mark_*` or `run_*` free functions after typestate migration even though they only proxy one method call.

## Step 6: Be Deliberate About State-Specific Data

### What to do

Attach data to a state variant only when that data is an invariant of that phase.

Examples:

- `InReview(ReviewData)` is good if review metadata is only meaningful during review.
- `Published(PublishMeta)` is good if publication metadata exists only after publishing.

If data is globally relevant (like `id`, tenant, repository handle), keep it on the machine struct.

### Why it matters

Correct placement turns the type system into a validator for data lifecycle. You prevent impossible combinations like "published document with no publish timestamp."

It also improves expressiveness: the state type itself documents which data is meaningful in that phase.

Ownership cost guideline:

- If transitions repeatedly clone large payloads, reevaluate placement.
- Move truly cross-phase data to machine context.
- Keep state payloads compact and phase-local.
- Prefer passing lightweight identifiers between states when full payload transfer is unnecessary.

### Common mistake

Using state data as a dumping ground for arbitrary payloads. If everything is attached to variants, the model becomes hard to evolve and reason about.

## Step 7: Rehydrate From Persistence With `#[validators]`

### What to do

When reconstructing from database rows or external records, use validators to map runtime facts back into typed machine states.

```rust
use statum::validators;

enum DbStatus {
    Draft,
    InReview,
    Published,
}

struct DbDocument {
    id: String,
    status: DbStatus,
}

#[validators(DocumentMachine)]
impl DbDocument {
    fn is_draft(&self) -> statum::Result<()> {
        match self.status {
            DbStatus::Draft => Ok(()),
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_in_review(&self) -> statum::Result<ReviewData> {
        match self.status {
            DbStatus::InReview => Ok(ReviewData { reviewer: "sam".into() }),
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_published(&self) -> statum::Result<PublishMeta> {
        match self.status {
            DbStatus::Published => Ok(PublishMeta { published_at_unix: 0 }),
            _ => Err(statum::Error::InvalidState),
        }
    }
}
```

Then build the machine with context:

```rust
let typed = row
    .into_machine()
    .id("doc-123".to_string())
    .storage(storage)
    .publisher(publisher)
    .build()?;
```

Async validator note:

- Validators may be sync or async.
- If any validator is async, generated machine builders are async too.
- Keep the validator style consistent within a type so call sites are predictable.

### Why it matters

Persistence is where type guarantees often degrade. Validators provide a controlled bridge from dynamic storage facts into a statically typed machine.

Done well, this improves correctness without hurting idiomaticity: runtime uncertainty stays at the boundary, typed invariants stay inside the core domain model.

### Common mistake

Treating persisted status as trusted and bypassing validation. That invites silent protocol drift and invalid state reconstruction.

## Step 8: Draw the Hybrid Boundary Explicitly

### What to do

Keep typestate for stable protocol edges. Keep runtime validation for domains that are inherently dynamic.

Good hybrid boundary:

- Core lifecycle phases in types.
- Policy-driven, user-authored, or plugin-defined choices at runtime.

Boundary worksheet (fill this before coding):

- Type-level core: edges that must never be violated.
- Runtime policy edge: edges controlled by tenant config, experiments, or external plugins.
- Rehydration boundary: all points where dynamic state is converted back into typed machine state.

### Why it matters

This keeps correctness where it pays most while avoiding over-modeling volatile behavior.

It also protects readability and extensibility. Teams can evolve dynamic policy logic without constantly refactoring type-level protocol code.

### Common mistake

Treating typestate adoption as all-or-nothing. Most production systems gain more from a clear boundary than from forcing type-level modeling into dynamic areas.

## Step 9: Evaluate Candidate Fit Quickly

Before implementing, run this compact checklist:

1. Can you list a finite set of meaningful states?
2. Are legal transitions mostly known at compile time?
3. Is invalid transition cost materially high?
4. Do methods differ by state in a meaningful way?
5. Does some data become valid/required only in specific states?
6. Is this lifecycle stable enough to justify type-level encoding?

Interpretation:

- 5-6 yes: strong typestate candidate.
- 3-4 yes: likely hybrid.
- 0-2 yes: runtime model likely better.

Escalation guidance:

- Strong candidate: model full core protocol in typestate first.
- Hybrid candidate: model "spine" states in typestate, keep optional branches runtime-validated.
- Runtime candidate: keep explicit validators and state-transition tests; revisit typestate if workflow stabilizes.

## Step 10: Testing and Acceptance Criteria

Typestate reduces many invalid-path tests, but it does not remove testing. Test the boundaries where runtime facts enter the system.

Minimum test set:

1. Happy-path transition sequence(s) for each main lifecycle.
2. Guard failure paths for runtime-checked edges (permission checks, missing data, feature gates).
3. Rehydration coverage for every persisted status variant.
4. Rollback or retry behavior where applicable.
5. One migration safety test if replacing an existing runtime model.

Acceptance criteria for adoption:

- Illegal transitions are unrepresentable in public API surface.
- Rehydration from persistence is centralized through validators.
- Team can explain the lifecycle by reading state names and transition method signatures only.
- Added type complexity is justified by reduced runtime validation noise.

When runtime transition tests are redundant:

- Remove tests that only assert impossible typed mis-orderings.
- Keep tests for boundary adapters, guards, side effects, and persistence/rehydration.

Quality acceptance check:

- Readability: reviewers can infer the lifecycle from state/transition names with minimal extra docs.
- Modularity: state behavior changes are mostly localized to one state impl block.
- Extensibility: adding one stable state does not require broad rewrites across unrelated states.
- Expressiveness: return types and method availability clearly encode protocol intent.
- Idiomaticity: ownership/borrowing patterns are straightforward and do not depend on hacks.
- Correctness: invalid protocol paths fail at compile time where feasible, else at explicit runtime boundaries.

## Step 11: Bon + Statum Composition

### What to do

Use `bon` for assembling input/context, and `statum` for enforcing protocol legality.

Guideline:

- Builder (bon): data assembly and defaults.
- Typestate (statum): ordered lifecycle and legal transitions.

```rust
let command = PostMessageCommand::builder()
    .sender(sender)
    .receiver(receiver)
    .body(body)
    .build();

let flow = PostMessageMachine::<Incoming>::from_command(command);
```

### Why it matters

This split improves idiomaticity and readability: builders solve construction ergonomics, typestate solves workflow legality.

### Common mistake

Using builders to simulate protocol steps that should be encoded as typed transitions.

## End-to-End Skeleton

This is a compact shape you can reuse:

```rust
use std::sync::Arc;
use statum::{machine, state, transition, validators};

#[state]
pub enum DocumentState {
    Draft,
    InReview(ReviewData),
    Published(PublishMeta),
}

pub struct ReviewData {
    reviewer: String,
}

pub struct PublishMeta {
    published_at_unix: i64,
}

trait Storage {}
trait Publisher {}

#[machine]
pub struct DocumentMachine<DocumentState> {
    id: String,
    storage: Arc<dyn Storage>,
    publisher: Arc<dyn Publisher>,
}

#[transition]
impl DocumentMachine<Draft> {
    fn submit_for_review(self, reviewer: String) -> DocumentMachine<InReview> {
        self.transition_with(ReviewData { reviewer })
    }
}

#[transition]
impl DocumentMachine<InReview> {
    fn publish(self, unix_ts: i64) -> DocumentMachine<Published> {
        self.transition_with(PublishMeta { published_at_unix: unix_ts })
    }
}

enum DbStatus {
    Draft,
    InReview,
    Published,
}

struct DbDocument {
    status: DbStatus,
}

#[validators(DocumentMachine)]
impl DbDocument {
    fn is_draft(&self) -> statum::Result<()> {
        matches!(self.status, DbStatus::Draft)
            .then_some(())
            .ok_or(statum::Error::InvalidState)
    }

    fn is_in_review(&self) -> statum::Result<ReviewData> {
        matches!(self.status, DbStatus::InReview)
            .then_some(ReviewData { reviewer: "sam".into() })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_published(&self) -> statum::Result<PublishMeta> {
        matches!(self.status, DbStatus::Published)
            .then_some(PublishMeta { published_at_unix: 0 })
            .ok_or(statum::Error::InvalidState)
    }
}
```

Skeleton expansion for branch routing:

```rust
enum PublishDecision {
    Published(DocumentMachine<Published>),
    StayInReview(DocumentMachine<InReview>),
}

impl DocumentMachine<InReview> {
    fn decide_publish(self, can_publish: bool, unix_ts: i64) -> PublishDecision {
        if can_publish {
            PublishDecision::Published(self.publish(unix_ts))
        } else {
            PublishDecision::StayInReview(self)
        }
    }
}
```

## Scenario Calibration

Use these to sanity-check your instincts:

1. Strong fit: payments state machine (`Authorized -> Captured -> Refunded`)
   - high correctness and compliance cost, clear legal edges.
2. Strong fit: content workflow (`Draft -> Review -> Publish`)
   - state-specific behavior and data are obvious.
3. Hybrid fit: onboarding with feature flags and experimentation
   - stable high-level phases, dynamic branch logic.
4. Weak fit: user-configurable workflow builder
   - transition graph defined at runtime by users/plugins.

## Practical Migration Path

If you already have a runtime enum/status model:

1. Keep current behavior.
2. Extract the most expensive invalid transitions.
3. Encode only that stable core with typestate.
4. Move state-specific methods into concrete state impl blocks.
5. Add validators for rehydration boundaries.
6. Expand only where value continues to exceed complexity.

This staged migration avoids big-bang rewrites while still delivering compile-time safety early.

## Anti-Patterns and Refactors

1. Anti-pattern: giant generic `impl<S>` with transition-like methods.
   - Refactor: move methods into concrete `impl Machine<StateX>` blocks.
2. Anti-pattern: "everything is state data."
   - Refactor: move cross-cutting fields to machine context.
3. Anti-pattern: "everything is machine context."
   - Refactor: encode phase-specific invariants in data-bearing variants.
4. Anti-pattern: skipping validators during rehydration.
   - Refactor: centralize conversion through `#[validators]` and builder flow.
5. Anti-pattern: typestate for volatile user-defined graphs.
   - Refactor: maintain runtime graph engine; use typed wrappers only around stable subflows.
6. Anti-pattern: forwarding-wrapper explosion (`mark_*`, `run_*`, `next_*`) that only delegates.
   - Refactor: call typed state methods directly; keep free functions only for real boundary adaptation.
7. Anti-pattern: non-transition helpers inside `#[transition]` impl blocks.
   - Refactor: move constructors/policy helpers into regular impl blocks and keep transition impls protocol-only.
8. Anti-pattern: policy branching hidden in procedural glue.
   - Refactor: return typed decision enums from concrete states and branch explicitly in orchestration.
9. Anti-pattern: clone-heavy transition payload churn.
   - Refactor: rebalance machine-context vs state-payload ownership to keep transitions lightweight.

## Design Review Checklist

Use this quick checklist during code review:

1. Are states finite, domain-named, and protocol-meaningful?
2. Are legal edges represented by methods on concrete source states?
3. Are `#[transition]` impl blocks free of constructors/helpers?
4. Are top-level helper functions limited to true boundary/policy concerns?
5. Are branch decisions represented explicitly (enum/result) instead of hidden glue?
6. Are ownership/clone costs acceptable across transitions?
7. Are tests focused on runtime boundaries rather than impossible typed mis-orders?

## Final Guidance

Yes, you should phrase it the way you described:

- identify the staged entity,
- define states first,
- encode with `#[state]`,
- define machine context with `#[machine]`,
- implement legal transitions with `#[transition]`,
- then add validators when crossing persistence boundaries.

That sequence is what keeps the model readable, modular, expressive, idiomatic, extensible, and correct.
