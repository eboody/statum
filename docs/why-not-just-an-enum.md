# When Not To Use Statum

A plain Rust enum is often the right starting point. It is small, obvious, and
cheap to persist. Use it until the workflow needs more than a label.

Statum starts to pay for itself when a phase should change the API surface: a
draft can be submitted, an in-review document can be approved, and the reviewer
assignment should only exist while review is active. At that point, an enum value
plus `match` statements can still work, but it keeps asking every caller to
remember the protocol.

This guide compares five choices:

- a plain enum and ordinary functions
- a runtime state machine around an enum
- a construction builder such as `derive_builder`, `typed-builder`, or `bon`
- ordinary runtime validation at service boundaries
- Statum's typestate machine generated with `#[state]`, `#[machine]`,
  `#[transition]`, and `#[validators]`

## The Short Version

| Concern | Plain enum | Runtime state machine | Statum typestate |
| --- | --- | --- | --- |
| API surface | Every value usually has the same methods or free functions. Illegal calls fail at runtime or are guarded by `match`. | A wrapper can centralize checks, but callers still hold one runtime type. Illegal calls usually return errors. | Each phase is a different Rust type, so only phase-legal methods are callable. |
| Phase-specific data | Usually stored as optional fields, side tables, or enum payloads that must be re-matched wherever they are used. | The wrapper can hide some details, but data is still reached through runtime branching. | State variant payloads become typed `state_data` on the phases that own them. |
| Persistence rehydration | Rows rebuild into raw enum/status values; every service boundary must decide what is legal next. | Rows rebuild into a runtime wrapper; methods validate again when called. | `#[validators]` rebuilds raw rows into `machine::SomeState`; handlers match once, then use phase-specific typed machines. |
| Introspection | You can hand-write a graph, but it is separate from the code that enforces behavior. | The machine may expose a runtime graph if you maintain one. | `MachineIntrospection::GRAPH` is generated from the machine and transition definitions. Strict mode narrows graph authority to directly readable transition signatures or explicit annotations. |
| Agent/tooling benefits | Agents must infer the protocol from scattered matches, comments, and tests. | Better if checks are centralized, but tools still see one broad runtime type. | Tools can inspect generated graph metadata and phase-specific method surfaces; prompts can ask for transitions, illegal edges, and rehydration boundaries directly. |

The decision point is not whether enums are good. They are. The question is
whether your public API should make illegal phases hard to express in the first
place.

The same boundary applies to builders and validators. Statum is not a better
`derive_builder`, `typed-builder`, `bon`, or validation framework. Those crates
and patterns are usually the better fit when the problem is constructing one
valid value or checking request data. Statum is for stable workflow protocols
where the allowed operations change after the value enters a new phase.

## Option 1: A Plain Enum

This is fine for a small workflow:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
enum DocumentStatus {
    Draft,
    InReview,
    Published,
}

struct Document {
    id: i64,
    title: String,
    body: String,
    status: DocumentStatus,
    reviewer: Option<String>,
}
```

The shape is honest. The workflow state is visible, serializable, and easy to
log.

The problem appears when callers start to encode protocol rules by convention:

```rust
fn approve(mut document: Document) -> Result<Document, &'static str> {
    if document.status != DocumentStatus::InReview {
        return Err("only documents in review can be approved");
    }

    document.status = DocumentStatus::Published;
    document.reviewer = None;
    Ok(document)
}
```

That guard is necessary, but it is not representational correctness. The type
`Document` still permits values that should not exist in the core API:

```rust
let broken = Document {
    id: 1,
    title: "Launch notes".to_owned(),
    body: "Ship it".to_owned(),
    status: DocumentStatus::Draft,
    // Broken: draft documents should not have a reviewer yet.
    reviewer: Some("ada".to_owned()),
};

// Broken: this call compiles. The mistake is only found at runtime.
let result = approve(broken);
```

Use this shape when the status is mostly descriptive, the method surface barely
changes by phase, or the workflow is private to one module.

## Option 2: A Runtime State Machine

A runtime machine can improve the plain enum by centralizing checks:

```rust
struct DocumentMachine {
    id: i64,
    title: String,
    body: String,
    status: DocumentStatus,
    reviewer: Option<String>,
}

impl DocumentMachine {
    fn submit(mut self, reviewer: String) -> Result<Self, &'static str> {
        if self.status != DocumentStatus::Draft {
            return Err("only drafts can be submitted");
        }

        self.status = DocumentStatus::InReview;
        self.reviewer = Some(reviewer);
        Ok(self)
    }

    fn approve(mut self) -> Result<Self, &'static str> {
        if self.status != DocumentStatus::InReview {
            return Err("only documents in review can be approved");
        }

        self.status = DocumentStatus::Published;
        self.reviewer = None;
        Ok(self)
    }
}
```

This is often a good production design. It gives you one place to put transition
rules and error messages.

It still exposes one broad runtime type, though. Every `DocumentMachine` value
has both methods, and illegal calls are still expressible:

```rust
let draft = DocumentMachine {
    id: 1,
    title: "Launch notes".to_owned(),
    body: "Ship it".to_owned(),
    status: DocumentStatus::Draft,
    reviewer: None,
};

// Broken: this compiles and depends on the method body to reject it.
let result = draft.approve();
```

Use this shape when dynamic workflows, user-authored graphs, or runtime policy
rules are more important than phase-specific compile-time APIs.

## Option 3: A Builder Crate

Use `derive_builder`, `typed-builder`, `bon`, or another construction builder
when the hard part is assembling one value correctly:

```rust
struct CreateDocumentRequest {
    title: String,
    body: String,
    reviewer_hint: Option<String>,
}
```

Builders are good at required fields, defaults, fluent call sites, fallible
construction, and ergonomic optional inputs. Typed builders can even make some
fields required at compile time.

That is a different problem from a workflow lifecycle. A builder normally
finishes when `build()` returns. It does not make `submit()` disappear after a
document leaves draft, add reviewer data only while review is active, or rebuild
a persisted row into one of several phase-specific machine types.

Use a builder crate when:

- the value has many fields, defaults, or optional inputs
- the legal operations after construction do not change much by phase
- you want a polished constructor, not a long-lived protocol type
- runtime errors from `build()` are acceptable for invalid input combinations

Use Statum beside a builder when the constructed value later enters a workflow
whose phases should expose different APIs. The boundary can be simple: let a
builder create request data or shared machine fields, then let Statum own the
phase transitions.

## Option 4: Runtime Validation

Runtime validation is the right tool when the rules are data-dependent,
tenant-configured, policy-heavy, or expected to change without a Rust release.
Examples include feature-flagged approval rules, custom field schemas,
permission checks, fraud scoring, and cross-record constraints from a database.

Statum should not absorb those rules just to make them look compile-time. The
type system can say that only an in-review document has an `approve()` method;
it cannot prove that the current user is allowed to approve this customer, that
the billing account is current, or that today's tenant policy still permits the
transition.

Use runtime validation when:

- the rule depends on the current user, clock, database, feature flag, or tenant
  policy
- invalid input should produce structured API errors instead of compiler errors
- non-Rust configuration defines the allowed shape
- the workflow graph itself is user-authored or changes at runtime

Statum and runtime validation can be combined. Use Statum for the stable phase
surface, then validate dynamic policy inside the transition method before it
returns the next typed machine.

## Option 5: Statum Typestate

Statum uses the enum as the source of phase names, then gives each phase a
different machine type:

```rust
use statum::{machine, state, transition};

#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
    Published,
}

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
```

Now the valid path is direct:

```rust
# use statum::{machine, state, transition};
# #[state]
# enum DocumentState {
#     Draft,
#     InReview(ReviewAssignment),
#     Published,
# }
# struct ReviewAssignment {
#     reviewer: String,
# }
# #[machine]
# struct Document<DocumentState> {
#     id: i64,
#     title: String,
#     body: String,
# }
# #[transition]
# impl Document<Draft> {
#     fn submit(self, reviewer: String) -> Document<InReview> {
#         self.transition_with(ReviewAssignment { reviewer })
#     }
# }
# #[transition]
# impl Document<InReview> {
#     fn approve(self) -> Document<Published> {
#         self.transition()
#     }
# }
let draft = Document::<Draft>::builder()
    .id(1)
    .title("Launch notes".to_owned())
    .body("Ship it".to_owned())
    .build();

let in_review = draft.submit("ada".to_owned());
let _published = in_review.approve();
```

The broken path no longer belongs to the method surface:

```rust,compile_fail
# use statum::{machine, state, transition};
# #[state]
# enum DocumentState {
#     Draft,
#     InReview(ReviewAssignment),
#     Published,
# }
# struct ReviewAssignment {
#     reviewer: String,
# }
# #[machine]
# struct Document<DocumentState> {
#     id: i64,
#     title: String,
#     body: String,
# }
# #[transition]
# impl Document<Draft> {
#     fn submit(self, reviewer: String) -> Document<InReview> {
#         self.transition_with(ReviewAssignment { reviewer })
#     }
# }
# #[transition]
# impl Document<InReview> {
#     fn approve(self) -> Document<Published> {
#         self.transition()
#     }
# }
let draft = Document::<Draft>::builder()
    .id(1)
    .title("Launch notes".to_owned())
    .body("Ship it".to_owned())
    .build();

// Broken: `approve` is not defined on `Document<Draft>`.
let _published = draft.approve();
```

State-specific data is also attached to the phase that owns it. A
`Document<InReview>` has review data; a `Document<Draft>` and
`Document<Published>` do not carry that payload.

## Persistence Is The Boundary

Most real workflows do not live entirely in memory. They cross database, event
log, queue, or HTTP boundaries.

With a plain enum or runtime wrapper, the rehydrated value is still a runtime
status. Callers must branch before doing useful work:

```rust
match row.status {
    DocumentStatus::Draft => submit_from_row(row)?,
    DocumentStatus::InReview => approve_from_row(row)?,
    DocumentStatus::Published => return Err("already published"),
}
```

With Statum, validators make the boundary explicit: raw rows stay raw until they
are proven to represent one known machine state.

```rust
use statum::{machine, state, validators};

#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
    Published,
}

struct ReviewAssignment {
    reviewer: String,
}

#[machine]
struct Document<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

enum DocumentStatus {
    Draft,
    InReview,
    Published,
}

struct DocumentRow {
    id: i64,
    title: String,
    body: String,
    status: DocumentStatus,
    reviewer: Option<String>,
}

#[validators(Document)]
impl DocumentRow {
    fn is_draft(&self) -> statum::Result<()> {
        matches!(self.status, DocumentStatus::Draft)
            .then_some(())
            .ok_or(statum::Error::InvalidState)
    }

    fn is_in_review(&self) -> statum::Result<ReviewAssignment> {
        if matches!(self.status, DocumentStatus::InReview) {
            let reviewer = self
                .reviewer
                .clone()
                .ok_or(statum::Error::InvalidState)?;
            Ok(ReviewAssignment { reviewer })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_published(&self) -> statum::Result<()> {
        matches!(self.status, DocumentStatus::Published)
            .then_some(())
            .ok_or(statum::Error::InvalidState)
    }
}
```

A handler can rebuild, match once, and then receive the phase-specific API:

```rust
# use statum::{machine, state, validators};
# #[state]
# enum DocumentState {
#     Draft,
#     InReview(ReviewAssignment),
#     Published,
# }
# struct ReviewAssignment {
#     reviewer: String,
# }
# #[machine]
# struct Document<DocumentState> {
#     id: i64,
#     title: String,
#     body: String,
# }
# enum DocumentStatus {
#     Draft,
#     InReview,
#     Published,
# }
# struct DocumentRow {
#     id: i64,
#     title: String,
#     body: String,
#     status: DocumentStatus,
#     reviewer: Option<String>,
# }
# #[validators(Document)]
# impl DocumentRow {
#     fn is_draft(&self) -> statum::Result<()> {
#         matches!(self.status, DocumentStatus::Draft)
#             .then_some(())
#             .ok_or(statum::Error::InvalidState)
#     }
#     fn is_in_review(&self) -> statum::Result<ReviewAssignment> {
#         if matches!(self.status, DocumentStatus::InReview) {
#             let reviewer = self
#                 .reviewer
#                 .clone()
#                 .ok_or(statum::Error::InvalidState)?;
#             Ok(ReviewAssignment { reviewer })
#         } else {
#             Err(statum::Error::InvalidState)
#         }
#     }
#     fn is_published(&self) -> statum::Result<()> {
#         matches!(self.status, DocumentStatus::Published)
#             .then_some(())
#             .ok_or(statum::Error::InvalidState)
#     }
# }
fn load(row: &DocumentRow) -> statum::Result<document::SomeState> {
    Document::rebuild(row)
        .id(row.id)
        .title(row.title.clone())
        .body(row.body.clone())
        .build()
}
```

That is the main reason to choose Statum over a plain enum: the boundary between
raw persisted facts and safe typed workflow values is visible in code.

## Introspection And Tooling

A plain enum can be documented, and a runtime machine can expose a hand-written
graph. Statum generates graph metadata from the machine and transition items
instead.

That gives humans and tools one more observation point:

- documentation can render the legal transition graph
- tests can assert that important edges exist
- CLIs can explain why an input row rebuilt into a particular state
- coding agents can ask for workflow phases, legal transitions, and rehydration
  boundaries without reverse-engineering every `match`

Be precise about the authority surface. `MachineIntrospection::GRAPH` is useful
machine metadata. With the `strict-introspection` feature, exact transition
claims are limited to directly readable `#[transition]` signatures and explicit
`#[introspect(return = ...)]` annotations. Unsupported shapes are rejected in
strict mode rather than guessed.

## Choosing The Smallest Tool

Use a plain enum when:

- the status mostly labels a value
- all phases expose nearly the same operations
- invalid transitions are cheap and local
- persistence is simple and callers already need to branch dynamically

Use a runtime state machine when:

- transition rules should be centralized
- the graph is dynamic or user-authored
- illegal calls should return domain errors instead of becoming type errors
- one broad runtime type is simpler for your API consumers

Use a builder crate when:

- the main problem is constructing one valid value
- required fields, defaults, and fluent call sites matter more than phase-specific methods
- `build()` is the lifecycle boundary
- the value does not need to become several different typed phases over time

Use runtime validation when:

- the rule depends on users, permissions, clocks, databases, feature flags, or tenant policy
- invalid input should become a structured runtime error
- rules are configured outside Rust code
- the workflow graph changes at runtime

Use Statum when:

- pressing `.` on one phase should show different operations than another phase
- phase-specific data should not exist outside its phase
- persisted facts should be rebuilt into typed states before handlers proceed
- generated graph metadata would help docs, tests, CLIs, or coding agents
- representational correctness is worth a typed API surface

Statum does not replace enums, builders, or validators. It uses an enum to
define a phase family, then moves the most important stable protocol rules into
Rust's type system.
