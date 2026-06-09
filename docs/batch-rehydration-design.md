# Batch Rehydration Helper Design

This note specifies the intended batch shape for typed rehydration. It documents
how `Machine::rebuild_many(...)`, `.into_machines()`, `.into_machines_by(...)`,
`.build()`, and `.build_reports()` should behave when a collection contains a
mix of valid and invalid rows.

The design goal is boring batch behavior: every input gets one output slot, slots
stay in input order, and callers can choose fail-fast, partial-success, or audit
report policies without losing per-row evidence.

## Authority Surface

The current observation point is each runtime input value passed through the
generated `#[validators]` rebuild surface. Batch helpers do not inspect storage,
SQL order clauses, event-stream completeness, or caller identity. They only
summarize the per-item rebuild reports produced from the values they receive.

## Existing Generated Shape

For a persisted type `TaskRow` with `#[validators(TaskMachine)]`, Statum exposes:

```rust
let results: Vec<statum::Result<task_machine::SomeState>> =
    TaskMachine::rebuild_many(rows)
        .client("acme".to_owned())
        .name("import".to_owned())
        .build();
```

and the report form:

```rust
let reports: Vec<statum::RebuildReport<task_machine::SomeState>> =
    TaskMachine::rebuild_many(rows)
        .client("acme".to_owned())
        .name("import".to_owned())
        .build_reports();
```

`rows.into_machines()` is the fallback equivalent for shared machine fields.
`rows.into_machines_by(|row| task_machine::Fields { ... })` supplies per-row
machine fields before calling `.build()` or `.build_reports()`.

If any validator is async, the same finalizers are async and should be awaited:

```rust
let reports = TaskMachine::rebuild_many(rows)
    .client("acme".to_owned())
    .name("import".to_owned())
    .build_reports()
    .await;
```

## Ordering Contract

Batch output order is stable and index-based:

- output index `i` corresponds to input index `i` after the provided collection is
  converted into the internal `Vec`;
- successful rows and failed rows are not regrouped;
- async validators may run concurrently, but the returned vector still preserves
  input order;
- `.into_machines_by(...)` computes the machine fields for each item before that
  item's rebuild, and the returned output slot still corresponds to the same
  input item.

This lets callers zip original ids with outcomes safely:

```rust
let ids: Vec<DocumentId> = rows.iter().map(|row| row.id).collect();
let reports = DocumentMachine::rebuild_many(rows)
    .db(db.clone())
    .build_reports()
    .await;

for (id, report) in ids.into_iter().zip(reports) {
    record_rebuild_outcome(id, report);
}
```

## Partial Failure Contract

Batch rebuilding is not all-or-nothing.

`.build()` returns one `Result` per input row:

- `Ok(machine)` means one validator accepted that row under normal rebuild
  semantics;
- `Err(statum::Error::InvalidState)` means no validator accepted that row;
- a failure in one slot does not discard successes in earlier or later slots.

`.build_reports()` returns one `RebuildReport` per input row:

- `report.result` carries the same success or failure that `.build()` would have
  returned for that row;
- `report.attempts` records the validators evaluated for that row in validator
  order;
- `report.ambiguity` remains `NotChecked`, because `build_reports()` preserves
  normal rebuild semantics and stops after the first accepted candidate.

Use `.build()` when the caller only needs typed machines and per-row errors. Use
`.build_reports()` when an import job, repair screen, migration audit, or admin
CLI needs to explain why each bad row failed.

## Aggregate Summary Helper

A small caller-side helper should sit on top of `Vec<RebuildReport<T>>` rather
than replacing it. The report vector is the lossless output; the summary is a
convenience view for dashboards, logs, and import responses.

Recommended shape:

```rust
struct BatchRebuildSummary<T> {
    rows: Vec<BatchRebuildRow<T>>,
    total: usize,
    succeeded: usize,
    failed: usize,
}

struct BatchRebuildRow<T> {
    index: usize,
    result: statum::Result<T>,
    report: statum::RebuildReport<T>,
}

impl<T> BatchRebuildSummary<T> {
    fn is_clean(&self) -> bool {
        self.failed == 0
    }

    fn successes(&self) -> impl Iterator<Item = (usize, &T)> {
        self.rows.iter().filter_map(|row| {
            row.result.as_ref().ok().map(|machine| (row.index, machine))
        })
    }

    fn failures(&self) -> impl Iterator<Item = (usize, &statum::RebuildReport<T>)> {
        self.rows.iter().filter_map(|row| {
            row.result.as_ref().err().map(|_| (row.index, &row.report))
        })
    }
}
```

If this helper becomes generated API, the name should stay close to the existing
vocabulary: `build_summary()`, `build_report_summary()`, or
`rebuild_many(...).build_reports().summarize()` are preferable to names like
`load_all` or `recover_many`.

## Caller Policies

The summary must not decide the application policy. It should make these policies
easy to express:

### Fail fast after preserving evidence

```rust
let batch = summarize_rebuild_reports(
    DocumentMachine::rebuild_many(rows).db(db.clone()).build_reports().await,
);

if !batch.is_clean() {
    return Err(import_failed(batch.failed));
}

let machines = batch.rows.into_iter()
    .map(|row| row.result)
    .collect::<statum::Result<Vec<_>>>()?;
```

### Accept partial success

```rust
let batch = summarize_rebuild_reports(
    DocumentMachine::rebuild_many(rows).db(db.clone()).build_reports().await,
);

for (index, machine) in batch.successes() {
    apply_next_step(index, machine);
}
for (index, report) in batch.failures() {
    quarantine_row(index, report);
}
```

### Return an import response

```rust
ImportResponse {
    total: batch.total,
    imported: batch.succeeded,
    rejected: batch.failed,
    rejected_rows: batch.failures()
        .map(|(index, report)| RejectedRow {
            index,
            reasons: report.attempts.iter()
                .filter_map(|attempt| attempt.reason_key.clone())
                .collect(),
        })
        .collect(),
}
```

## Non-Goals

- Do not hide invalid rows by returning only `Vec<T>`.
- Do not reorder results into successes first and failures later.
- Do not make batch helpers own database transactions or retries.
- Do not use `recover` in the generated helper name; recovery is caller policy
  after rebuild evidence exists.
- Do not claim that batch success proves the backing store is complete or current.

## Acceptance Rules

Any implementation of an aggregate helper should preserve these rules:

1. One input item produces one row in the lossless output.
2. Output order matches input order.
3. Partial failures are represented, not thrown away.
4. Aggregate counts are derived from per-row outcomes.
5. Per-row reports remain available for diagnostics.
6. Async and sync batch finalizers expose the same result shape, with async only
   changing whether the caller awaits the finalizer.
