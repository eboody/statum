# `statum-macros` Architecture

`statum-macros` is organized as a small compiler pipeline. The crate is larger
than the runtime crates because most of its work happens at compile time:
parsing user syntax, resolving machine/state identities, validating contracts,
rendering diagnostics, and emitting the public API surface.

The design goal is reviewability rather than minimum line count. Each macro
should make its semantic stage visible in the directory tree so a reviewer can
see where authority is claimed and where code generation starts.

## Pipeline stages

Every macro should follow the same staged model:

1. **Parse** raw Rust syntax into Statum-owned inputs.
2. **Resolve** machine, state, module, and source-backed identities.
3. **Validate contracts** against the resolved semantic model.
4. **Prepare diagnostic facts** from already observed/resolved data.
5. **Emit** generated tokens from validated semantic structures.

The stages are allowed to reject unsupported inputs, but public claims must match
their observation point. For example, strict transition introspection is exact
only for return shapes that the macro directly resolves or that the user provides
with an explicit `#[introspect(return = ...)]` override.

## Authority surfaces

| Stage | Observation point | May source-scan? | May fail closed? | May emit runtime/API tokens? |
| --- | --- | ---: | ---: | ---: |
| Parse | Parsed Rust syntax (`syn`) | No | Yes | No |
| Resolve | Parsed syntax + macro registries + source queries | Yes, via `source/` only | Yes | No |
| Contract validation | Resolved Statum semantic model | No fresh scans | Yes | No |
| Diagnostics | Supplied diagnostic facts | No fresh scans | No new behavior | No |
| Emission | Validated semantic model | No | Internal-error only | Yes |

Treat words like "exact", "authoritative", "source of truth", and "no drift" as
proof obligations. If the available observation point is weaker than the claim,
narrow the claim or reject the unsupported case.

## Dependency direction

- `source/` must not depend on `state`, `machine`, `transition`, or
  `validators`.
- macro subsystems may depend on `source/` and `contracts.rs`.
- parse/resolve/contract layers may prepare data for diagnostics.
- diagnostics render facts supplied by earlier stages; they should not perform
  fresh source scans or broaden fail-closed behavior.
- emission code may depend on resolved contracts and parsed metadata, but not on
  raw source helpers.

## Subsystem ownership

- `src/source/`
  Owns file analysis, module-path lookup, source fingerprints, source-backed
  candidate queries, and alias/source shape observations. This layer reports raw
  or parsed-source facts; it does not claim expanded-macro or type-checked
  authority.

- `src/state/`
  Owns `#[state]` parsing, validation, registry storage, and generated
  marker/data surfaces.

- `src/machine/`
  Owns `#[machine]` parsing, validation, registry storage, transition support
  traits, builders, machine-state surfaces, presentation output, and
  introspection output.

- `src/transition/`
  Owns `#[transition]` impl parsing, machine resolution, return-shape contract
  validation, strict-vs-relaxed introspection policy, transition diagnostics,
  and transition registration emission.

- `src/validators/`
  Owns `#[validators(...)]` machine-path resolution, validator coverage and
  signature checking, typed rebuild helper emission, and batch rebuild surfaces.

## Preferred module shape

Favor semantic modules over helper buckets:

```text
transition/
  mod.rs          # thin orchestration and public exports
  parse.rs        # ItemImpl -> TransitionImpl
  resolve/        # machine/source lookup strategies
  contract/       # return target and wrapper contract validation
  validation.rs   # resolved transition method validation
  emit.rs         # token emission from validated contracts
  diagnostics.rs  # rendering only

machine/
  mod.rs          # thin orchestration and public exports
  validation.rs
  registry.rs
  emission/
    builders/     # machine construction builders
    presentation.rs
    support.rs

validators/
  mod.rs          # thin orchestration and public exports
  resolution/
  contract.rs
  plan.rs
  emission/
    rebuild.rs    # single-row rebuild surface
    batch.rs      # batch rebuild surface
    shared.rs
```

Avoid names like `helpers.rs`, `utils.rs`, or `stuff.rs` unless the module is
truly cross-cutting and has a narrow documented purpose.

## Testing expectations

- Behavior-preserving refactors should run both plain and `strict-introspection`
  macro suites when they touch transition parsing, contracts, diagnostics, or
  emission.
- Compile-fail diagnostics require committed `.stderr` fixtures.
- Adversarial tests should cover inputs whose meaning differs across observation
  stages: `#[cfg]`, macro-generated items, `include!`, duplicate machine names,
  aliases, and branch-return pressure.
- Emission refactors must keep generated API examples and `statum-examples`
  tests passing.
