# Persistence Integration Examples

Statum does not ship database, HTTP, or serialization adapters. The examples in
`statum-examples` show where application code should draw those boundaries:
fetch or deserialize raw data, rebuild it with validators, match the typed state,
call a legal transition, then persist and return the new representation.

## SQLx Row Boundary

Use this shape when a handler or worker owns a `sqlx` pool and a row table:

1. Fetch one row with `sqlx::query_as`.
2. Rebuild the row with `DocumentMachine::rebuild(&row)`.
3. Match `document_machine::SomeState` and call only the transition available on
   that typed state.
4. Persist the transitioned machine back through normal SQL.
5. Fetch or render the stored representation for the caller.

The runnable example is
[`statum-examples/src/showcases/axum_sqlite_review.rs`](../statum-examples/src/showcases/axum_sqlite_review.rs).
The `submit_via_sqlx_boundary` function is intentionally small so the adapter
boundary is visible: SQLx owns the pool, schema, query parameters, and update;
Statum owns only the validator-backed rebuild and legal typed transition.

Run it with:

```bash
cargo test -p statum-examples axum_sqlite_review
cargo run -p statum-examples --bin axum-sqlite-review
```

## Serde Snapshot Boundary

Use this shape when the durable form is a JSON document, cache entry, message, or
API payload:

1. Deserialize the snapshot into an application-owned struct.
2. Rebuild it with `CartMachine::rebuild(&snapshot)`.
3. Match the typed state and call the legal transition.
4. Serialize the new snapshot back to the store.
5. Return a response type built from the persisted snapshot, not from unchecked
   status strings.

The runnable example is
[`statum-examples/src/showcases/serde_json_snapshot.rs`](../statum-examples/src/showcases/serde_json_snapshot.rs).
`JsonSnapshotStore::checkout` demonstrates the full deserialize -> rehydrate ->
legal checkout -> serialize path without any database dependency.

Run it with:

```bash
cargo test -p statum-examples serde_json_snapshot
```

## Axum Handler Boundary

Use this shape when HTTP routes sit in front of persisted workflow state:

1. Validate request-only fields at the HTTP edge.
2. Delegate persisted-row work to a small SQLx boundary function.
3. Map missing rows, invalid requests, corrupt persisted state, and illegal
   transitions to explicit HTTP statuses.
4. Return a response DTO instead of exposing the typed machine directly.

The same
[`axum_sqlite_review.rs`](../statum-examples/src/showcases/axum_sqlite_review.rs)
showcase wires this through Axum routes. The handler does not branch on raw
`status` strings; it branches once on `document_machine::SomeState` after
validator-backed rehydration.

Run the HTTP path with:

```bash
cargo test -p statum-examples axum_sqlite_review_happy_path
```

## Boundary Responsibilities

Statum owns:

- generated typed machines and transition methods
- validator-backed rebuild from raw persisted data
- `SomeState` wrappers and optional rebuild reports

Application code owns:

- database schema, SQL, transactions, and connection pools
- JSON formats and Serde derives
- HTTP request and response DTOs
- concurrency checks and stale-write prevention
- business-specific error mapping

Keeping that split visible avoids turning Statum into a persistence framework
while still making the dynamic-to-typed boundary executable and testable.
