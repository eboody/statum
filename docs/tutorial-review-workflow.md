# Tutorial: Build A Review Workflow

This is the missing middle between the toy examples and the larger showcase
apps. It walks through the core shape of
[axum_sqlite_review.rs](../statum-examples/src/showcases/axum_sqlite_review.rs)
step by step.

Keep the main idea in view while reading: the goal is not to wrap a workflow in
extra types. The goal is to make invalid or not-yet-validated document states
impossible to treat as ordinary domain values.

What you are building:

- a `Document` that starts as a draft
- can move into review with an assigned reviewer
- can be approved into published
- is rehydrated from a database row before each HTTP transition

## 1. Start With The Legal States

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
```

This is more than an enum of labels.

Statum will generate:

- a marker type for each variant: `Draft`, `InReview`, `Published`
- a typed machine like `DocumentMachine<Draft>`
- a machine-scoped wrapper enum for reconstructed values:
  `document_machine::State`

## 2. Put Durable Fields On The Machine

```rust
#[machine]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
    body: String,
}
```

These are the fields that exist across the workflow. State-specific data does
not go here. `ReviewAssignment` only exists when the document is actually in
review, so it lives on `InReview`.

Review data is only present on `InReview` because that is the only state where
it is valid.

## 3. Define The Legal Edges

```rust
#[transition]
impl DocumentMachine<Draft> {
    fn submit(self, reviewer: String) -> DocumentMachine<InReview> {
        self.transition_with(ReviewAssignment { reviewer })
    }
}

#[transition]
impl DocumentMachine<InReview> {
    fn approve(self) -> DocumentMachine<Published> {
        self.transition()
    }
}
```

This is where the API becomes state-aware:

- `submit(...)` only exists on `DocumentMachine<Draft>`
- `approve()` only exists on `DocumentMachine<InReview>`
- `DocumentMachine<Published>` has neither method

This is the important shift from simple wrappers: the type controls which
methods exist in each phase.

## 4. Define The Persisted Shape

The app stores rows in SQLite:

```rust
#[derive(Clone, Debug, FromRow)]
struct DocumentRow {
    id: i64,
    title: String,
    body: String,
    status: String,
    reviewer: Option<String>,
}
```

By itself, this is just runtime data. It is not yet a typed workflow.
It may describe one legal state, or it may describe an invalid combination.

## 5. Rebuild Rows Into Typed Machines

This is the part that makes the example more than “states and transitions.”
It is the boundary where raw rows either become one legal typed state or remain
invalid runtime data.

```rust
#[validators(DocumentMachine)]
impl DocumentRow {
    fn is_draft(&self) -> statum::Result<()> {
        if *id > 0
            && !title.is_empty()
            && !body.is_empty()
            && self.status == STATUS_DRAFT
            && self.reviewer.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_review(&self) -> statum::Result<ReviewAssignment> {
        if *id <= 0 || title.is_empty() || body.is_empty() || self.status != STATUS_IN_REVIEW {
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
            && self.status == STATUS_PUBLISHED
            && self.reviewer.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}
```

Each validator says: “this row is a legal instance of this state.”

Notice two things:

- row data still comes from `self.status` and `self.reviewer`
- machine fields like `id`, `title`, and `body` are available by name inside
  the validator body

Now you can rebuild one row into a typed state:

```rust
async fn load_document_state(
    pool: &SqlitePool,
    id: i64,
) -> Result<document_machine::State, AppError> {
    let row = fetch_document_row(pool, id).await?;

    row.clone()
        .into_machine()
        .id(row.id)
        .title(row.title)
        .body(row.body)
        .build()
        .map_err(|_| AppError::CorruptState)
}
```

That `build()` call returns:

- `document_machine::State::Draft(DocumentMachine<Draft>)`
- `document_machine::State::InReview(DocumentMachine<InReview>)`
- or `document_machine::State::Published(DocumentMachine<Published>)`

## 6. Match Once At The Handler Boundary

This is the service-shaped part of the example.

```rust
async fn submit_document(...) -> Result<Json<DocumentResponse>, AppError> {
    let machine = load_document_state(&app.pool, id).await?;

    let machine = match machine {
        document_machine::State::Draft(machine) => machine.submit(request.reviewer),
        _ => {
            return Err(AppError::invalid_transition(
                "submit requires a draft document",
            ));
        }
    };

    persist_in_review(&app.pool, &machine).await?;
    ...
}
```

And approval is symmetric:

```rust
async fn approve_document(...) -> Result<Json<DocumentResponse>, AppError> {
    let machine = load_document_state(&app.pool, id).await?;

    let machine = match machine {
        document_machine::State::InReview(machine) => machine.approve(),
        _ => {
            return Err(AppError::invalid_transition(
                "approve requires an in-review document",
            ));
        }
    };

    persist_published(&app.pool, &machine).await?;
    ...
}
```

The important part is not the `match` itself. The important part is what you
get after the match:

- once you matched `Draft`, the compiler lets you call `submit(...)`
- once you matched `InReview`, the compiler lets you call `approve()`
- you cannot accidentally call `approve()` on a draft machine

## 7. Why This Example Matters

If you stopped at `#[state]` plus `#[transition]`, it would still be easy to
say “this is just a nicer wrapper.”

This example shows the larger shape Statum is for:

- state-specific data with `InReview(ReviewAssignment)`
- state-specific methods like `submit(...)` and `approve()`
- typed rebuild from stored rows with `#[validators]`
- one explicit workflow boundary at the HTTP handler

That is where Statum starts to pay for itself.

## 8. Where To Look Next

- Full source:
  [statum-examples/src/showcases/axum_sqlite_review.rs](../statum-examples/src/showcases/axum_sqlite_review.rs)
- Validator details:
  [persistence-and-validators.md](persistence-and-validators.md)
- Stronger persistence story:
  [case-study-event-log-rebuild.md](case-study-event-log-rebuild.md)
- Smaller minimal example:
  [example-editorial-workflow.md](example-editorial-workflow.md)
