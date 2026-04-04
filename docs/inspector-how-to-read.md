# How To Read The Inspector

The inspector has three different views because it answers three different
questions:

- `Journeys`: what happens from entry to exit, in order?
- `Machines`: what states and transitions are legal inside one machine?
- `Topology`: which machines are connected to which other machines?

If you want runtime story, start on `Journeys`. If you want legality, use
`Machines`. If you want workspace context, use `Topology`.

## Journeys

`Journeys` is the main composition view.

Read it like this:

- left top list: which composition machine you are exploring
- left bottom list: one exact finite journey for that machine
- center diagram: only the selected journey, not the whole machine
- right `Steps` rail: the same journey flattened into ordered step cards

Important rules:

- a journey is one exact finite root-to-sink composition trace
- the numbered transition labels in the diagram match the numbered step cards
- the center diagram is state order inside the selected composition machine
- the step rail shows zero or more exact cross-machine targets for each step
- `carries` means the composition state still holds a child protocol machine in
  that state after the step
- `targets` means that step handed off to or referenced another machine on that
  transition

When journeys are heavily branching:

- the journey list groups exact variants by `Entry -> Exit`
- `h` / `l` in the journey list switches endpoint families
- `j` / `k` picks a concrete exact variant inside the selected family
- jumping to `Topology` from here starts in a local focused neighborhood around
  the selected machine

## Machines

`Machines` is the legal-state drilldown.

Read it like this:

- center diagram: the full exact `stateDiagram-v2` for one machine
- `States`: per-state detail
- `Transitions`: legal moves out of each state
- `Relations`: exact handoffs and optional weaker hints
- `Journeys`: only present for composition machines

Use `Machines` when the question is:

- what states exist?
- what transitions are allowed?
- what relation is attached to this specific state or transition?

## Topology

`Topology` is not a runtime sequence view.

It shows whole machines and exact inter-machine links across the workspace or
the current neighborhood around the selected machine.

Read it like this:

- each box is one whole machine, not one state
- double-box node: composition machine
- plain box: protocol machine
- thick arrow: composition-owned handoff
- solid arrow: other linked handoff
- dotted arrow: static machine reference

Label rules:

- `owns xN`: `N` exact composition-owned links between those two machines
- `handoff xN`: `N` exact linked handoffs between those two machines
- `ref xN`: `N` exact static references between those two machines

Those counts are grouped link counts, not time order and not step counts.

So if you see:

```text
release_to_vendor ==> broker | owns x3
```

that means:

- `release_to_vendor` and `broker` are linked exactly three times
- the topology view does not tell you which one happens first
- the topology view does not tell you the transition names

Use `Enter` on a selected topology machine to jump back into `Journeys` or
`Machines`.

## Which View To Use

Use this shortcut:

- “What happens?” -> `Journeys`
- “What is legal?” -> `Machines`
- “What is connected?” -> `Topology`

If `Topology` feels confusing, that usually means you are asking a journey
question and should switch back to `Journeys`.
