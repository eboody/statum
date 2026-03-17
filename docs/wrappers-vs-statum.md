# Why Statum Instead Of Plain Wrappers?

This is the short answer to a fair question:

> Why is this better than regular type wrappers with private constructors and
> explicit transition methods?

A simple wrapper can enforce a valid starting state. A regular builder can make
construction ergonomic. Those solve different problems than typestate.

- A regular builder answers: how do I construct this value ergonomically? If
  that is the problem, use a builder crate like `bon`.
- A typestate builder answers: which construction steps are legal next, and can
  `build()` even exist yet?
- A typestate machine answers: once the value exists, which operations are
  legal in this state, and which next states are representable at all?

Statum is for the second and third categories. It helps when the builder itself
needs typestate guarantees, and when the resulting machine API should also be
state-aware. It is not trying to be a general-purpose builder crate.

So the real comparison is not:

- wrapper with private constructor
- versus macro-generated builder

The real comparison is:

- manually maintained per-state wrappers, `PhantomData` typestate, and custom
  rebuild code
- versus generated typestate machinery that keeps the state enum,
  transitions, builders, and rehydration surface aligned

The wrapper or regular-builder alternative usually still relies on the caller
to know what can be called when. It can restrict construction, but it usually
does not make the post-construction API state-aware unless you hand-build
per-state types.

The point is correctness. Some workflows are best modeled as a sequence of
states where only certain methods can exist in each phase. Statum lets you
encode that model directly and have the API enforced at compile time.

That is where it starts paying for itself: multi-step workflows with
state-specific data, legal transitions, and reconstruction from rows or event
logs. For a tiny in-memory flow, handwritten wrappers are still simpler.

For a concrete example, see
[../statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
and [case-study-event-log-rebuild.md](case-study-event-log-rebuild.md).
