# Macro UI Fixtures

This directory contains `trybuild` fixtures for the public macro surface. These
fixtures are not auto-discovered: every fixture must be registered explicitly in
`../macro_errors.rs` under the matching feature-gated test.

## Naming convention

- `invalid_*` fixtures are expected to fail compilation and need a matching
  `.stderr` file.
- `valid_*` fixtures are expected to compile in the default non-strict suite.
- `strict_invalid_*` and `strict_valid_*` fixtures belong to the
  `strict-introspection` feature-gated suite unless they intentionally also run
  in the default suite as compatibility coverage.
- `support/` and `workspace_member/` files are helper crates/modules used by the
  top-level fixtures. Do not register helper files directly unless they are the
  fixture entry point.

## Diagnostic quality bar

First-party Statum diagnostics should prefer this shape:

```text
Error: <what is wrong>, including the relevant state enum, machine, transition,
       validator, or presentation owner when available.
Found: <the user's input>
Expected: <the accepted shape>
Fix: <one concrete repair>
```

Native Rust errors are acceptable when the fixture intentionally exercises the
ordinary generated Rust surface, for example missing methods, private-field
access, duplicate builder setters, or removed legacy helper traits. If a native
Rust error obscures the Statum concept being tested, prefer adding a first-party
macro diagnostic and update the `.stderr` fixture.

## Fixture groups

### State and presentation shape

Registered in `test_invalid_state_usage`:

- `invalid_state_*` covers invalid `#[state]` targets, enum shapes, generics,
  cfg-sensitive variants, and state payload collisions.
- `invalid_presentation_*` covers `#[present(...)]` parsing and typed metadata
  requirements. These should include owner context such as `state
  FlowState::Draft`, `machine Flow`, or `transition Flow<Draft>::submit` when
  that context is available.

Positive coverage:

- `valid_state_*`
- `valid_presentation_sugar.rs`
- `valid_presentation_typed_metadata.rs`

### Machine shape and builders

Registered in `test_invalid_machine_usage`:

- `invalid_machine_*` covers invalid `#[machine]` targets, generic/state-family
  discovery, declaration order, private fields, machine attributes, and builder
  collision cases.

Positive coverage includes:

- `valid_machine_*`
- `valid_builder_usage.rs`
- `valid_visibility_and_reconstruction.rs`
- `valid_multiple_machines_same_module.rs`

### Transition attributes and transition resolution

Registered in `test_invalid_transition_attribute_usage`,
`test_invalid_transition_usage`, and `test_invalid_transition_usage_strict`:

- `invalid_transition_attr_args.rs` and `invalid_transition_introspect_*` cover
  attribute parsing.
- `invalid_transition_*` covers method shape, machine/source/return-state
  resolution, branch extraction, transition maps, cfg ambiguity, macro-generated
  aliases, `include!`, and legacy-surface absence.
- `strict_invalid_transition_*` covers strict-introspection cases where Statum
  must reject weaker source observations unless the user provides explicit
  introspection metadata.

Positive coverage includes:

- `valid_transition_*`
- `strict_valid_transition_*`

### Validators and typed rehydration

Registered in `test_invalid_validators_attribute_usage`,
`test_invalid_validators_usage`, and `test_invalid_validators_usage_strict`:

- `invalid_validators_*` covers invalid `#[validators]` attributes, missing or
  mismatched state validators, bad signatures, receiver misuse, state/machine
  resolution, parameter-name collisions, declaration order, and alias handling.
- `invalid_rebuild_*` covers typed rebuild/batch builder collisions.
- `strict_invalid_validators_*` covers strict-introspection path-resolution
  limitations.

Positive coverage includes:

- `valid_validators_*`
- `valid_into_machines_by.rs`
- `strict_valid_validators_*`

### Legacy absence checks

`invalid_legacy_*` fixtures intentionally assert that older generated helper
surfaces remain absent. They often rely on native Rust errors rather than custom
macro diagnostics.

## Updating stderr files

When intentionally changing diagnostics:

```bash
TRYBUILD=overwrite cargo test -p statum-macros
cargo test -p statum-macros
cargo test -p statum-macros --features strict-introspection
```

Review the generated `.stderr` files by hand before committing. Do not bless
changed diagnostics just because `TRYBUILD=overwrite` produced them.
