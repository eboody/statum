# Escape Hatches

This page is the grep-able audit list for Statum APIs that bypass a normal proof path or let callers supply metadata the macro cannot derive on its own.

Scary names are intentional. If an API skips validation, makes a caller assertion stand in for proof, or overrides an introspection fact, the name and docs must make the invariant transfer visible.

## Current Public Escape Hatches

### `#[introspect(return = ...)]`

`#[introspect(return = ...)]` is a transition introspection escape hatch. It is not an unchecked rehydration API and it does not construct a typed state.

Use it only when a `#[transition]` method's written return type is not directly usable as the strict graph contract, but the method still returns the same machine path in a supported wrapper shape.

Caller obligations:

- The annotation must name the real transition return shape for that method.
- The written return type must still resolve to the impl target machine path, or to a supported wrapper around it.
- The primary success branch must be the machine state that runtime callers actually receive when the transition succeeds.
- Unsupported custom decision enums, imported aliases, macro-generated aliases, include-generated aliases, ambiguous aliases, foreign machine paths, and wrapper aliases remain unsupported in strict mode.

Supported strict shapes are direct `Machine<NextState>` returns plus canonical wrappers around that same machine path:

- `::core::option::Option<Machine<NextState>>`
- `::core::result::Result<Machine<NextState>, E>`
- `::statum::Branch<Machine<Left>, Machine<Right>>`

If those obligations do not hold, rewrite the transition signature instead of using the annotation.

## Current Rehydration Escape Hatches

No current public API named `unchecked`, `assume_state`, or
`from_parts_unchecked` exists for typed rehydration.

The current public typed-rehydration path is validator-backed:

- `Machine::rebuild(&row)`
- `row.into_machine()`
- `Machine::rebuild_many(rows)`
- `.into_machines()`
- `.into_machines_by(...)`
- `.build_report()` / `.build_reports()`

Those APIs run validators. They are not unchecked construction APIs.

## Reserved Names For Future Rehydration Hatches

Future APIs that bypass validator proof must use grep-able names from this list:

- `unchecked` for general validation bypass.
- `assume_state` when the caller asserts the state marker from external proof.
- `from_parts_unchecked` when raw machine fields and state data are assembled without validators.

Do not use soft names such as `restore`, `load_state`, or `from_row` for validator bypass. Those names hide the proof boundary.

Any future unchecked rehydration API must document these caller obligations next to the API:

- The asserted state marker matches the real persisted workflow phase.
- State-specific data satisfies the same invariants the validator would have enforced.
- Shared machine fields belong to the same workflow instance as the state data.
- Downstream code may call state-specific transitions immediately, so a false assertion turns invalid persisted facts into ordinary typed workflow values.
- The API is for tests, migrations, repair, benchmarks, or legacy interop; it is not the normal rehydration path.

If an API cannot state those obligations plainly, it should not be an unchecked API.

## Audit Rule

When adding or reviewing an escape hatch, run:

```bash
bash scripts/check_escape_hatches.sh
```

The check does not prove semantic correctness. It keeps the public audit vocabulary present and catches accidental soft names for unchecked construction.
