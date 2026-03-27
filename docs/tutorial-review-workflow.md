# Tutorial: Grow A Review Workflow One Feature At A Time

This tutorial keeps one `Document` workflow and upgrades it in small, working
steps.

Use the build-up on purpose. Start with the smallest machine that works, then
add the next feature because the workflow now needs it.

By the end you will have the core shape behind
[axum_sqlite_review.rs](../statum-examples/src/showcases/axum_sqlite_review.rs):

- typed states
- durable machine fields
- explicit transitions
- state-specific data
- typed rehydration from stored rows
- one match at the service boundary

If you want the runnable examples alongside this doc, keep these files open:

- [example_01_setup.rs](../statum-examples/src/toy_demos/example_01_setup.rs)
- [04-transitions.rs](../statum-examples/src/toy_demos/04-transitions.rs)
- [08-transition-with-data.rs](../statum-examples/src/toy_demos/08-transition-with-data.rs)
- [09-persistent-data.rs](../statum-examples/src/toy_demos/09-persistent-data.rs)
- [13-review-flow.rs](../statum-examples/src/toy_demos/13-review-flow.rs)

## 1. Start With The Smallest Working Machine

First requirement: a document starts as a draft and may eventually be
published.

```rust
use statum::{machine, state};

#[state]
enum DocumentState {
    Draft,
    Published,
}

#[machine]
struct Document<DocumentState> {}

fn main() {
    let _draft = Document::<Draft>::builder().build();
}
```

This is deliberately small.

It already gives you two different types:

- `Document<Draft>`
- `Document<Published>`

But it is still weak as a real example. There are no fields, no legal moves,
and no reason to prefer this over a plain builder yet.

## 2. Add Durable Machine Context

Next requirement: a document is not just a state label. It has fields that
exist for the whole workflow.

```rust
use statum::{machine, state};

#[state]
enum DocumentState {
    Draft,
    Published,
}

#[machine]
struct Document<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

fn main() {
    let _draft = Document::<Draft>::builder()
        .id(1)
        .title("RFC: Typed review workflow".to_owned())
        .body("Start small, then add features.".to_owned())
        .build();
}
```

These are durable machine fields. They are present in every state.

That distinction matters later:

- durable workflow context goes on the machine
- state-specific data goes on the state variant that actually needs it

## 3. Add The First Legal Transition

Next requirement: publishing should consume a draft and produce a published
document.

```rust
use statum::{machine, state, transition};

#[state]
enum DocumentState {
    Draft,
    Published,
}

#[machine]
struct Document<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

#[transition]
impl Document<Draft> {
    fn publish(self) -> Document<Published> {
        self.transition()
    }
}

fn main() {
    let draft = Document::<Draft>::builder()
        .id(1)
        .title("RFC: Typed review workflow".to_owned())
        .body("Start small, then add features.".to_owned())
        .build();

    let _published = draft.publish();
}
```

This is the first real payoff:

- `publish()` exists on `Document<Draft>`
- `publish()` does not exist on `Document<Published>`

The legal move is now part of the type system instead of an `if status ==
"draft"` check.

## 4. Make The Workflow Real: Add Review And State Data

New requirement: drafts cannot publish directly anymore. They must go through
review, and the assigned reviewer should only exist while the document is in
review.

```rust
use statum::{machine, state, transition};

#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
    Published,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReviewAssignment {
    reviewer: String,
}

#[machine]
struct Document<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

#[transition]
impl Document<Draft> {
    fn submit(self, reviewer: String) -> Document<InReview> {
        self.transition_with(ReviewAssignment { reviewer })
    }
}

impl Document<InReview> {
    fn reviewer(&self) -> &str {
        &self.state_data.reviewer
    }
}

#[transition]
impl Document<InReview> {
    fn approve(self) -> Document<Published> {
        self.transition()
    }
}

fn main() {
    let draft = Document::<Draft>::builder()
        .id(1)
        .title("RFC: Typed review workflow".to_owned())
        .body("Start small, then add features.".to_owned())
        .build();

    let review = draft.submit("alice".to_owned());
    assert_eq!(review.reviewer(), "alice");

    let _published = review.approve();
}
```

This is the point where Statum stops looking like a thin wrapper.

`ReviewAssignment` is not an optional machine field that happens to be filled
in sometimes. It only exists on `Document<InReview>`, because that is the only
state where it is valid.

Now the API shape matches the workflow:

- `submit(...)` only exists on drafts
- `reviewer()` only exists during review
- `approve()` only exists during review
- published documents do not carry stale review data

## 5. Add Validators So Stored Rows Must Prove Their State

New requirement: documents come back from storage. A row with `status` and
`reviewer` fields is still raw data until it proves it matches one legal
state.

```rust
use statum::{machine, state, transition, validators};

#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
    Published,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReviewAssignment {
    reviewer: String,
}

#[machine]
struct Document<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

#[transition]
impl Document<Draft> {
    fn submit(self, reviewer: String) -> Document<InReview> {
        self.transition_with(ReviewAssignment { reviewer })
    }
}

#[transition]
impl Document<InReview> {
    fn approve(self) -> Document<Published> {
        self.transition()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Status {
    Draft,
    InReview,
    Published,
}

#[derive(Clone, Debug)]
struct DocumentRow {
    id: i64,
    title: String,
    body: String,
    status: Status,
    reviewer: Option<String>,
}

#[validators(Document)]
impl DocumentRow {
    fn is_draft(&self) -> statum::Result<()> {
        if *id > 0
            && !title.is_empty()
            && !body.is_empty()
            && self.status == Status::Draft
            && self.reviewer.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_review(&self) -> statum::Result<ReviewAssignment> {
        if *id <= 0 || title.is_empty() || body.is_empty() || self.status != Status::InReview {
            return Err(statum::Error::InvalidState);
        }

        self.reviewer
            .clone()
            .filter(|reviewer| !reviewer.trim().is_empty())
            .map(|reviewer| ReviewAssignment { reviewer })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_published(&self) -> statum::Result<()> {
        if *id > 0
            && !title.is_empty()
            && !body.is_empty()
            && self.status == Status::Published
            && self.reviewer.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

fn load_document_state(row: DocumentRow) -> statum::Result<document::SomeState> {
    row.clone()
        .into_machine()
        .id(row.id)
        .title(row.title)
        .body(row.body)
        .build()
}
```

This is the persistence boundary.

A few details matter:

- `self.status` and `self.reviewer` still come from the raw row
- `id`, `title`, and `body` are generated bindings for machine fields
- `is_in_review()` returns `ReviewAssignment` because that state carries data
- `load_document_state(...)` returns the generated wrapper enum `document::SomeState`
  and `document::State` remains an alias for compatibility

That last part is important. Rehydration does not guess a concrete machine
type. It returns one typed value out of the legal set:

- `document::SomeState::Draft(Document<Draft>)`
- `document::SomeState::InReview(Document<InReview>)`
- `document::SomeState::Published(Document<Published>)`

If none of the validators match, `.build()` returns
`statum::Error::InvalidState`.

## 6. Turn The Wrapper Enum Into A Concrete Document

At this point you do not yet have one concrete `Document<S>`. You have
`document::SomeState`, which means "one legal typed document out of this set."

The next step is to resolve that wrapper enum into the concrete machine you
need:

```rust
fn load_draft_document(row: DocumentRow) -> statum::Result<Document<Draft>> {
    match load_document_state(row)? {
        document::SomeState::Draft(machine) => Ok(machine),
        _ => Err(statum::Error::InvalidState),
    }
}

fn load_in_review_document(row: DocumentRow) -> statum::Result<Document<InReview>> {
    match load_document_state(row)? {
        document::SomeState::InReview(machine) => Ok(machine),
        _ => Err(statum::Error::InvalidState),
    }
}
```

That is the moment where the generic wrapper becomes a concrete machine:

- `Document<Draft>` if you need draft-only behavior
- `Document<InReview>` if you need review-only behavior
- `Document<Published>` if you need published-only behavior

Once you have one of those concrete types, the compiler exposes only the API
that is legal for that state.

## 7. Match Once At The Boundary, Then Use Typed Methods

Final requirement: a handler or service function should branch once on the
rehydrated state, then operate on the concrete typed machine.

```rust
fn submit_document(row: DocumentRow, reviewer: String) -> statum::Result<Document<InReview>> {
    match load_document_state(row)? {
        document::SomeState::Draft(machine) => Ok(machine.submit(reviewer)),
        _ => Err(statum::Error::InvalidState),
    }
}

fn approve_document(row: DocumentRow) -> statum::Result<Document<Published>> {
    match load_document_state(row)? {
        document::SomeState::InReview(machine) => Ok(machine.approve()),
        _ => Err(statum::Error::InvalidState),
    }
}
```

This is the service-shaped payoff:

- stored data is validated before it becomes a typed machine
- you match once at the boundary
- after the match, the compiler exposes only the methods that are legal there

That is the core Statum story. The type system does not replace your workflow.
It makes the legal workflow explicit and executable.

## 8. How The Rest Of The Feature Set Fits

Once this progression clicks, the other features are easier to place:

- [06-async-transitions.rs](../statum-examples/src/toy_demos/06-async-transitions.rs):
  transition logic can do async work before returning the next typed machine
- [09-persistent-data.rs](../statum-examples/src/toy_demos/09-persistent-data.rs):
  if a validator is `async`, the generated `.build()` becomes `async`
- [14-batch-machine-fields.rs](../statum-examples/src/toy_demos/14-batch-machine-fields.rs):
  rebuild many rows when each one needs different machine fields
- [15-transition-map.rs](../statum-examples/src/toy_demos/15-transition-map.rs):
  declare legal transition edges up front
- [17-attested-composition.rs](../statum-examples/src/toy_demos/17-attested-composition.rs):
  carry exact child-transition provenance into a parent transition and inspect
  the resulting relation in the linked codebase graph
- [sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs):
  append-only events projected back into typed machine states

If you want the full app version of this tutorial, read
[axum_sqlite_review.rs](../statum-examples/src/showcases/axum_sqlite_review.rs)
after this doc. It uses the same core idea, just with HTTP and SQLite around
it.

If the next question is “how does this show up in the graph and inspector
tools?”, read [introspection.md](introspection.md) next. That guide covers the
exact machine graph, the linked codebase relation graph, and the attested
composition surface.
