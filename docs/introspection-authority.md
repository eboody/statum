# Introspection Authority Boundaries

This page is the semantic boundary for Statum graph metadata. Use it when a
README, generated artifact, CLI output, or agent prompt needs to say what the
metadata knows and what it cannot know.

Short version: Statum graph metadata is not a type-checker, an expanded Rust
program, a runtime trace, or a persistence audit. Version 1 graph metadata
claims the workflow structure Statum can derive from the active cfg-pruned
attribute-macro input and the supported syntax listed below.

## Observation Stages

| Stage | Does graph metadata observe it? | What this means |
| --- | --- | --- |
| Raw source text | No | Metadata is not produced by scanning files or grep-able source text. Comments, inactive `#[cfg]` branches, and text outside macro inputs are not graph facts. |
| Parsed AST / macro item input | Partly | Statum parses the item input given to each relevant attribute macro with `syn`. Claims are limited to syntax the macro explicitly supports. |
| Cfg-pruned AST | Yes, for macro inputs | Rust has already removed whole items disabled by `#[cfg]` before the active build reaches the attribute macros. Whole disabled items are absent from the graph. |
| Expanded items | No | Attribute macros do not inspect arbitrary code generated later by other macros. A transition site must be visible to Statum before Statum's macro observes it, or it is not part of the metadata claim. |
| Type-checked items | No | Graph metadata does not inspect rustc's resolved type graph. It relies on syntactic paths and supported return-shape parsing, then the generated Rust must still compile. |
| Runtime registry / runtime values | Only as a carrier | `MachineGraph` and `StableGraphMetadata` values can be read at runtime, and runtime transition records can join to generated ids. Runtime choices do not add static graph edges. |
| Persisted state | No | Rehydration and validators inspect persisted values at runtime. Stable graph metadata does not claim that a database row, event log, or JSON snapshot is valid. |

## Surface-by-Surface Authority

| Metadata surface | Claimed authority | Actual observation point | Unsupported or rejected cases |
| --- | --- | --- | --- |
| `MachineGraph` states | Active states for the macro-observed machine | Cfg-pruned `#[state]` enum variants parsed from macro input | Nested `#[cfg]` or `#[cfg_attr]` on variants and payload fields are rejected because they would make the active graph ambiguous. |
| `MachineGraph` machine fields | Shared machine shape needed by generated builders and projection helpers | Cfg-pruned `#[machine]` struct fields parsed from macro input | Nested `#[cfg]` or `#[cfg_attr]` on machine fields are rejected. Field-level graph metadata is reserved but not populated in stable metadata v1. |
| Transition source states | Source state for each transition impl block | Cfg-pruned `#[transition]` impl self type parsed from macro input | Transition impls hidden behind inactive whole-item `#[cfg]` gates are absent. Transition sites generated outside Statum's observed macro input are absent. |
| Transition target states, strict mode | Legal target states for each supported transition method site | Cfg-pruned transition method signatures plus supported return wrappers: direct `Machine<NextState>`, `::core::option::Option<...>`, `::core::result::Result<..., E>`, `::statum::Branch<..., ...>`, or explicit `#[introspect(return = ...)]` | Custom decision enums, wrapper aliases, differently qualified machine paths, body-only branches, and runtime-only targets are rejected instead of approximated. |
| Transition target states, default mode | Useful graph targets for ergonomic source shapes | The same macro input, with a wider alias-following fallback where current relaxed introspection supports it | This mode is useful metadata, but do not call it exact-authority metadata. Use strict mode when the public claim requires fail-closed target authority. |
| Transition identity | Stable ids for source-state plus method sites visible to Statum | Generated ids from the cfg-pruned transition macro input | Duplicate or ambiguous ids must be resolved by macro validation before a graph is emitted. Runtime-only transitions cannot introduce new ids. |
| Source-local presentation | Labels, descriptions, and typed metadata attached to machines, states, and transitions | Cfg-pruned `#[present(...)]` and `#[presentation_types(...)]` attributes parsed from macro input | Presentation is human-facing metadata, not proof of workflow legality. Typed metadata categories must provide `metadata = ...` where required; otherwise generation rejects the item. |
| `StableGraphMetadata` JSON | Stable serialized graph document for tooling | A lowering of the generated `MachineGraph` plus presentation fields into the v1 schema | The JSON records `authority: "cfg_pruned_macro_input"` and explicit `unsupported_cases`; consumers must not infer stronger authority from the serialization format. |
| Mermaid, DOT, and matrix renderers | Deterministic renderings of the stable metadata document | The already-produced `StableGraphMetadata` value | Renderers do not re-inspect source, type-check code, validate persistence, or discover hidden transitions. Unknown targets stay visible as placeholders when the metadata contains them. |
| Graph invariant lints | Heuristic warnings over an exported graph document | The serialized/exported `StableGraphMetadata` values | Runtime policy, guard conditions, validator predicates, domain vocabulary, and rejected transition sites are outside the lint pass. Clean lint output is not a semantic proof. |

## Safe Public Wording

Use wording like this:

- "Metadata is derived from the active cfg-pruned macro input for supported
  syntax."
- "Strict introspection rejects unsupported return shapes instead of guessing
  transition targets."
- "Graph renderers operate on `StableGraphMetadata`; they do not re-inspect Rust
  source or claim stronger authority than the metadata document."
- "Runtime transition records join actual choices back to generated transition
  ids; they do not add new static graph edges."

Avoid or qualify wording like this:

- "Statum knows every possible runtime branch." It does not observe arbitrary
  function bodies or runtime values.
- "The JSON is the source of truth for the Rust program." It is a stable export
  of generated metadata, not rustc's type-checked program model.
- "Graph lints prove workflow correctness." They are heuristics over exported
  metadata only.
- "Default-mode introspection is exact." The exact-authority claim belongs to
  strict mode plus supported syntax or explicit introspection overrides.

## Closeout Checklist For Metadata Work

Before shipping a metadata, renderer, lint, or docs change, state all of these in
the handoff:

- Claimed authority surface.
- Actual observation point.
- Unsupported cases rejected or still open.
- Adversarial tests or docs cases covering `#[cfg]`, generated items, custom
  wrappers, `include!`, duplicate ids, runtime-only transitions, or persisted
  state, as applicable to the change.
