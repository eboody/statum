# Presentation Attribute Sugar Plan

This is an optional follow-up plan for adding metadata authoring sugar on top of
Statum's existing typed introspection and presentation overlay APIs.

Implementation status:

- Stage 1 is shipped.
- Stage 2 typed metadata is also shipped.
- In typed categories declared through `#[presentation_types(...)]`, each
  annotated item must provide `metadata = ...` so the generated
  `machine::PRESENTATION` constant stays fully typed without inventing default
  values.

## Recommendation

Implement this in two stages.

Start with prose-only sugar and keep the current typed overlay as the real
model. That means labels and descriptions can live near the machine
definition, but the generated output is still just a
`MachinePresentation<..., (), (), ()>` constant. If that proves useful and not
noisy, add a second opt-in layer for typed `metadata = expr` payloads.

## Surface

### Stage 1

```rust
#[machine]
#[present(label = "Flow", description = "Validation flow")]
struct Flow<FlowState> {}

#[state]
enum FlowState {
    #[present(label = "Fetched", description = "Ready for validation")]
    Fetched,
    #[present(label = "Accepted")]
    Accepted,
    #[present(label = "Rejected")]
    Rejected,
}

#[transition]
impl Flow<Fetched> {
    #[present(label = "Validate", description = "Choose accepted or rejected")]
    fn validate(self) -> Result<Flow<Accepted>, Flow<Rejected>> {
        ...
    }
}
```

Generated output:

- `flow::PRESENTATION: MachinePresentation<flow::StateId, flow::TransitionId>`
- emitted only when at least one `#[present(...)]` is used
- `GRAPH` stays unchanged

### Stage 2

Only add this if real usage shows the manual overlay is too repetitive.

```rust
#[presentation_types(
    machine = crate::meta::MachineMeta,
    state = crate::meta::StateMeta,
    transition = crate::meta::TransitionMeta,
)]
#[machine]
#[present(label = "Flow", metadata = crate::meta::MachineMeta::Flow)]
struct Flow<FlowState> {}
```

Then variants and transition methods can use `metadata = <expr>` too, and the
macro emits a typed `flow::PRESENTATION` constant using those declared metadata
types.

## Implementation Plan

1. Add a small shared parser in `statum-macros` for `#[present(...)]`.
   It should accept only `label`, `description`, and later `metadata`.
   Reject unknown keys and duplicates with precise spans.

2. For stage 2 only, add `#[presentation_types(...)]` on the machine struct.
   That keeps type declarations in one place instead of repeating them on every
   state and transition.

3. Extend existing parsers to retain presentation attrs.
   - machine struct attrs for machine-level presentation
   - `#[state]` enum variant attrs for state presentation
   - `#[transition]` method attrs for transition presentation

4. Reuse the ids that Statum already emits.
   Generate `flow::PRESENTATION` in the same machine module that already owns
   `StateId`, `TransitionId`, and `GRAPH`.
   Do not add a second graph model.

5. Keep emission conditional.
   If there are no presentation attrs, emit nothing. That keeps the sugar from
   adding namespace noise for users who do not want it.

6. Keep consumer overlays first-class.
   Do not make generic code depend on macro-emitted presentation. Manual
   `MachinePresentation` values should remain fully supported and equally
   legitimate.

7. Add tests in this order.
   - prose-only pass case
   - repeated labels and descriptions across states and transitions
   - duplicate-key and unknown-key compile errors
   - typed `metadata = expr` pass case
   - missing `#[presentation_types]` when `metadata = expr` is used
   - integration test joining `GRAPH`, `PRESENTATION`, and a recorded runtime
     event

## Guardrails

- no layout or renderer concerns in the attrs
- no impact on legality or transition generation
- no required metadata
- no stringly replacement for the typed overlay
- no second DSL beyond a thin attribute veneer over the existing model

## Stopping Point

If this gets implemented, stop after stage 1 unless a real consumer
immediately needs typed metadata sugar. That is where the feature is most
likely to start feeling noisy.
