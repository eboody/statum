# Diagnostics Quality Audit

Statum's proc-macro diagnostics are part of the public API. They should teach
the user the model while rejecting unsupported input.

This audit captures the current quality bar and the next polish targets. It is
based on the committed `statum-macros/tests/ui/*.stderr` fixtures.

## Diagnostic Standard

A first-party Statum diagnostic should usually include:

```text
Error: what was rejected, with the relevant domain name
Found: the shape Statum observed
Expected: the supported shape
Fix: the smallest concrete edit that moves the user forward
```

Use the domain name whenever possible:

- `#[state]` enum name,
- `#[machine]` struct name,
- source/target state variant,
- transition method,
- validator method,
- generated rebuild surface.

Not every compiler error needs this structure. Some compile-fail tests verify
that removed legacy APIs or intentionally absent generated methods stay absent.
Those may remain ordinary Rust errors.

## Current Strengths

Many first-party diagnostics already follow the desired shape. Good examples
include:

- state shape errors such as `invalid_state_tuple_variant.stderr`,
- machine generic errors such as `invalid_machine_no_state_generic.stderr`,
- transition resolution errors such as `invalid_transition_unknown_machine.stderr`,
- validator return-shape errors such as `invalid_validators_wrong_return.stderr`,
- strict-introspection errors that name the unsupported authority surface.

These are the diagnostics to preserve during refactors.

## Accepted Rust-Compiler Fallbacks

The following categories may intentionally produce native Rust errors instead of
custom Statum diagnostics:

- legacy API absence checks, such as removed helper traits or old builder names,
- duplicate setter calls where typestate builder state removes the method after
  first use,
- private field access checks that verify generated visibility boundaries,
- generated trait-bound failures that prove an undeclared transition-map edge is
  unavailable.

These tests are valuable, but their stderr fixtures should be clearly named so
reviewers know the compiler error is intentional.

## Polish Targets

### 1. Builder duplicate-setter errors

Current duplicate setter tests prove the type-level builder state works, but the
visible error is a method-not-found message on an internal generated builder
type. That is correct behavior, but not yet luxury UX.

Target:

- keep the compile-time rejection,
- add documentation explaining that duplicate setter calls intentionally remove
  the setter from the builder state,
- consider a future custom lint-like diagnostic only if it can be implemented
  without weakening the typestate guarantee.

Fixtures:

- `invalid_machine_builder_duplicate_field.stderr`,
- `invalid_machine_builder_duplicate_state_data.stderr`,
- `invalid_rebuild_builder_duplicate_field.stderr`,
- `invalid_rebuild_many_builder_duplicate_field.stderr`.

### 2. Presentation diagnostics

Presentation errors are already concrete, but some messages do not name the
owning machine/state/transition when the attribute appears on a domain item.

Target:

- include owner context where available,
- keep the current `Found`/`Expected`/`Fix` structure.

Fixtures:

- `invalid_presentation_duplicate_key.stderr`,
- `invalid_presentation_missing_parens.stderr`,
- `invalid_presentation_unknown_key.stderr`,
- `invalid_presentation_metadata_without_types.stderr`.

### 3. Legacy API failures

Legacy absence tests currently rely on native Rust errors. That is acceptable if
the purpose is compatibility enforcement, but the docs should not suggest these
are first-class onboarding diagnostics.

Target:

- keep them as regression tests,
- avoid treating them as examples of desired user-facing errors.

Fixtures:

- `invalid_legacy_machine_builder.stderr`,
- `invalid_legacy_machines_builder.stderr`,
- `invalid_legacy_superstate.stderr`,
- `invalid_legacy_transition_helper_trait.stderr`,
- `invalid_legacy_state_helper_traits.stderr`.

## Release Checklist

Before a diagnostics-focused release:

1. Run `TRYBUILD=overwrite cargo test -p statum-macros` only after intentionally
   changing diagnostics.
2. Inspect every changed `.stderr` fixture by hand.
3. Confirm each new first-party diagnostic names the relevant domain item.
4. Confirm unsupported exact-authority cases fail closed rather than guessing.
5. Run both macro suites:

```bash
cargo test -p statum-macros
cargo test -p statum-macros --features strict-introspection
```
