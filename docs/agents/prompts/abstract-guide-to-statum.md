# Abstract Guide To Statum Prompt

Copy and paste this when you want an agent to turn an architecture memo,
protocol guide, critique, or design plan into concrete Statum machines.

```text
Turn this guide into the strongest Statum design that still respects hybrid boundaries.

Inspect the guide for:
- staged entities
- trust or protocol boundaries
- evidence and approval requirements
- hard rejects or failure states
- persistence, replay, or rebuild boundaries
- child workflows that should become nested machines
- governance flows that should be separate machines
- dynamic policy that should stay runtime-validated

Tasks:
1. Classify each candidate workflow as strong, hybrid, or poor fit.
2. For each strong or hybrid candidate, propose:
   - machine name and role
   - a `#[state]` enum
   - a `#[machine]` struct with shared context
   - the state-specific data
   - the legal `#[transition]` impl blocks
   - any parent, child, or nested machine relationships
   - whether `#[validators]` or `statum::projection` belongs at a persistence boundary
3. Call out what should explicitly stay outside Statum and why.
4. Keep the first implementation slice small enough for one focused PR.
5. If a claim is underspecified, name the missing protocol or evidence question instead of guessing.

Return:
## Fit
## Machine inventory
## Hybrid boundary
## Persistence and rebuild
## First implementation slice
## Tests and open questions

Be concrete. Do not stop at "consider typestate."

Use these references if needed:
- https://github.com/eboody/statum/blob/main/README.md
- https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md
- https://github.com/eboody/statum/blob/main/docs/patterns.md
- https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md
```
