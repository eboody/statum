# Public Rehydration Vocabulary

This note defines the public words Statum should use around typed rehydration:
`rebuild`, `rehydrate`, `recover`, `project`, `explain`, and explicit
unchecked escape hatches.

The goal is not to invent six overlapping APIs. The goal is to give each word a
small job so docs, examples, diagnostics, and future APIs do not drift.

## Authority Surface

Typed rehydration claims are about one observation point: a runtime input value
that is passed through the generated `#[validators]` rebuild surface.

That means Statum can say:

- a persisted row, projection, document, or payload was accepted by one validator
  and rebuilt into one generated typed machine wrapper, or
- no validator accepted that value and the rebuild failed.

Statum should not claim that storage itself is correct, complete, or the source
of truth. Storage remains an external system. Event projection remains caller
code plus `statum::projection` helpers. Validators are the boundary where those
runtime facts are interpreted as one legal machine state.

## Summary Table

| Term | Meaning | Use when |
| --- | --- | --- |
| `project` | Fold an event stream or external shape into a row-like snapshot that validators can inspect. | The source data is not already one validator input value. |
| `rebuild` | Run validators against one or more input values and produce typed machine states, per-row failures, or reports. | Naming concrete Statum entrypoints such as `Machine::rebuild`, `rebuild_many`, `.build()`, `.build_report()`, or `.build_reports()`. |
| `rehydrate` | The broader product story: move stored/runtime facts back into typed workflow values. | Explaining the feature in prose, docs headings, and user-facing mental models. |
| `recover` | Resume useful work after rebuild failure by choosing an application policy. | The caller handles invalid, missing, ambiguous, stale, or partial data. |
| `explain` | Return or display why a rebuild would accept or reject candidate states. | Debugging, CLI output, admin UI, migration audits, or report-oriented APIs. |
| unchecked / `assume_state` | Bypass validator proof and construct or coerce a typed state from caller assertions. | Rare migration, test, repair, or interop code where the caller accepts the invariant burden. |

## `project`

`project` means reducing data that is not already a single validator input into a
shape that can be validated.

Projection does not make a typed machine. It prepares facts for typed
rehydration.

Use `project` when:

- reading append-only events and reducing them to the latest snapshot;
- grouping interleaved event streams into one row per workflow id;
- normalizing an external webhook or log shape into the persisted type that owns
  the `#[validators(Machine)]` impl.

Current API examples:

```rust
let row = statum::projection::reduce_one(events, &OrderProjector)?;
let machine = OrderMachine::rebuild(&row).build()?;
```

```rust
let rows = statum::projection::reduce_grouped(events, |event| event.order_id, &OrderProjector)?;
let machines = rows.into_machines().build();
```

Do not use `project` for the validator pass itself. If a row is already ready for
`Machine::rebuild(&row)`, projection is done.

## `rebuild`

`rebuild` is the precise API word for running generated validators and producing
typed machine values.

A rebuild attempt starts with one input value plus any required machine fields.
It ends with either:

- one generated wrapper enum variant, such as `task_machine::SomeState::Draft`,
  carrying the concrete typed machine for the accepted state; or
- `statum::Error::InvalidState` when no validator accepts the value.

Use `rebuild` when naming concrete entrypoints:

```rust
let machine = TaskMachine::rebuild(&row)
    .client("acme".to_owned())
    .name("spec".to_owned())
    .build()?;
```

```rust
let machines = TaskMachine::rebuild_many(rows)
    .client("acme".to_owned())
    .build();
```

Batch rebuilds keep one output slot per input slot. Successes and failures remain
in input order, so callers can build aggregate summaries without losing per-row
evidence. See [batch-rehydration-design.md](batch-rehydration-design.md) for the
partial-failure and ordering contract.

```rust
let reports = TaskMachine::rebuild_many(rows)
    .client("acme".to_owned())
    .build_reports();
```

```rust
let report = TaskMachine::rebuild(&row)
    .client("acme".to_owned())
    .name("spec".to_owned())
    .build_report();
```

Prefer `rebuild` over vague verbs like `load` or `parse` in API names. `load`
suggests I/O ownership; `parse` suggests syntax recognition. Rebuild is the
runtime boundary where validator methods prove membership in a state family.

## `rehydrate`

`rehydrate` is the broader feature word. It describes the user problem: turning
stored or runtime facts back into typed workflow values so ordinary code no
longer carries raw status rows.

Use `rehydrate` in prose:

- "typed rehydration from SQLite rows";
- "rehydrate before handling a transition request";
- "append-only projection followed by typed rehydration".

Use `rebuild` for the API call that performs it:

```rust
// Prose: rehydrate the stored row before approving it.
let machine = DocumentMachine::rebuild(&row).db(db.clone()).build()?;
```

This split keeps docs readable without making the API vocabulary fuzzy:
`rehydration` is the capability; `rebuild` is the concrete operation.

## `recover`

`recover` means an application policy after rebuild did not produce the typed
state needed for normal work.

Recovering is not the same as rebuilding. Rebuild answers "does this input match
one legal state?" Recovery answers "what should this app do now?"

Use `recover` for caller-owned paths such as:

- quarantine the row and show an operator repair screen;
- choose a rollback event after projection fails;
- skip invalid rows during a batch job but emit a report;
- map `InvalidState` to an HTTP 409, 422, or admin-only diagnostic response;
- migrate old rows, then retry `rebuild`.

Example policy shape:

```rust
match TaskMachine::rebuild(&row).client(client).name(name).build_report() {
    report if report.result.is_ok() => report.into_result(),
    report => recover_invalid_task(row.id, report),
}
```

Do not name normal validator entrypoints `recover`. That would imply Statum can
repair data by itself. Statum can surface the failed rebuild; caller policy owns
the repair.

## `explain`

`explain` means exposing the evidence from a rebuild attempt without requiring a
caller to infer it from one success or failure value.

The current explainable surfaces are:

- `.build_report()`, which keeps normal rebuild semantics and stops once the
  first validator accepts;
- `.explain()`, which evaluates every candidate validator and returns a
  `RebuildReport` without throwing away per-candidate rejection details.

Both surfaces use `RebuildReport`:

- `RebuildReport::attempts` records validator attempts in evaluation order;
- `RebuildAttempt::matched` marks accepted validators;
- validators that return `statum::Validation<T>` or
  `Result<T, statum::Rejection>` can populate `reason_key` and `message` for
  rejected candidates;
- `RebuildReport::ambiguity` is `NotChecked` for `.build_report()` and
  `Unambiguous` or `Ambiguous { matched_states }` for `.explain()`;
- `RebuildReport::into_result()` returns the ordinary rebuild result.

Use `explain` for tooling and diagnostics:

```rust
let report = TaskMachine::rebuild(&row)
    .client("acme".to_owned())
    .name("spec".to_owned())
    .explain();

for attempt in &report.attempts {
    eprintln!(
        "{} -> {} matched={} reason={:?}",
        attempt.validator,
        attempt.target_state,
        attempt.matched,
        attempt.reason_key,
    );
}
```

Future APIs may use names like `explain_rebuild`, `explain_candidate`, or
`candidate_report` if they return report data without changing normal rebuild
semantics. They should not silently accept invalid input; explain mode describes
candidate evaluation, not an escape hatch.

## Unchecked / `assume_state` Escape Hatches

Unchecked or `assume_state` APIs mean the caller asserts the state invariant
without running validators.

The current audit catalog lives in [escape-hatches.md](escape-hatches.md). It
records the existing `#[introspect(return = ...)]` metadata escape hatch and the
fact that Statum does not currently expose unchecked typed-rehydration
constructors.

They are intentionally different from `rebuild`:

- `rebuild` proves membership by executing validators at the runtime boundary;
- unchecked construction assumes membership because the caller says it is true.

Use an unchecked escape hatch only when the surrounding context already provides
a stronger proof or when no typed proof is possible yet:

- narrowly scoped tests that need to set up an impossible fixture;
- one-off data repair or migration code that has performed its own audit;
- interop with a legacy subsystem that already enforces the same workflow
  invariant;
- benchmark fixtures where validation is not the behavior under measurement.

Public naming should be scary and grep-able. Prefer names that contain one of:

- `unchecked`, for bypassing validation generally;
- `assume_state`, for asserting a specific state from external proof;
- `from_parts_unchecked`, for constructing from raw machine fields and state
  data.

Avoid soft names like `restore`, `load_state`, or `from_row` for these APIs.
Those names hide the invariant transfer.

Unchecked docs should always say what the caller must uphold. Example wording:

```rust
// Hypothetical future shape, not current API:
let machine = unsafe {
    TaskMachine::<InReview>::from_parts_unchecked(fields, review_data)
};
```

Caller obligations for an API like that would be:

- the state marker must match the actual persisted workflow phase;
- state-specific data must satisfy the same invariants that validators would
  require;
- shared machine fields must describe the same workflow instance;
- downstream code may call state-specific transitions immediately, so a wrong
  assertion can turn invalid data into ordinary typed workflow values.

Unchecked APIs should be documented as migration and repair tools, not normal
rehydration tools.

## Recommended Public Language

Use these pairings consistently:

- "Project events into rows, then rebuild typed machines."
- "Typed rehydration is the feature; `Machine::rebuild` is the entrypoint."
- "Use rebuild reports to explain which validator accepted or rejected a row."
- "Recovery is caller policy after failed rebuild, not a replacement for
  validators."
- "Unchecked or `assume_state` APIs bypass proof and transfer the invariant burden
  to the caller."

Avoid these pairings:

- "project into a typed machine" — projection stops at row-like facts;
- "recover the state" when the operation only runs validators;
- "explain mode rebuilds invalid data" — reports explain rejection; they do not
  make it valid;
- "unchecked rehydration" without naming the skipped proof.

## Closeout Checklist For Future API Work

When implementing or documenting a new rehydration API, close the vocabulary
loop explicitly:

1. Is the input already one validator input? If not, call the preceding fold a
   projection.
2. Does the operation run validators and produce typed machines? Name it
   rebuild.
3. Is the text describing the product capability instead of one function call?
   Rehydration is fine.
4. Does the caller decide what to do after failure? Call that recovery.
5. Does the API return candidate attempts, rejection keys, or messages? It is an
   explain/report surface.
6. Does it bypass validators? Put `unchecked` or `assume_state` in the name and
   document caller obligations.
