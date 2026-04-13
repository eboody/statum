# `statum-macros` Architecture

`statum-macros` is organized as three internal subsystems:

1. Source observation
   This layer reads files, reconstructs module paths, and answers source-backed lookup questions. It lives under `src/source/`.

2. Semantic resolution
   This layer turns parsed syntax plus observed source facts into machine, state, transition, and validator contracts. Shared contract types live in `src/contracts.rs`.

3. Emission and diagnostics
   This layer renders generated tokens and compile errors from already parsed or resolved inputs. It should not perform fresh source scans.

## Dependency direction

- `source/` must not depend on `state`, `machine`, `transition`, or `validators`.
- macro subsystems may depend on `source/` and `contracts.rs`.
- emission code may depend on resolved contracts and parsed metadata, but not on raw source helpers.
- diagnostics should explain existing fail-closed behavior, not broaden it.

## Subsystem ownership

- `src/source/`
  Owns file analysis, module-path lookup, source fingerprints, and source-backed candidate queries.

- `src/state/`
  Owns `#[state]` parsing, validation, registry storage, and generated marker/data surfaces.

- `src/machine/`
  Owns `#[machine]` parsing, validation, registry storage, transition support traits, builders, machine-state surfaces, and introspection output.

- `src/transition/`
  Owns `#[transition]` impl parsing, source-backed return-shape resolution, strict-vs-relaxed introspection policy, diagnostics, and transition registration emission.

- `src/validators/`
  Owns `#[validators(...)]` machine-path resolution, validator coverage and signature checking, and typed rebuild helper emission.
