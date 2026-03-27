# External Typestate Exemplars

This note is a curated shortlist of external Rust repositories worth reading if
the goal is to learn both:

- quality code generally
- typestate builder and typestate API design specifically

It complements the internal
[Typestate Builder Design Playbook](typestate-builder-design-playbook.md). That
playbook is about how to design our own APIs. This document is about which
external codebases are worth imitating and what they teach.

The list is short by design. Many repos use typestate somewhere. Fewer are
clean enough, mature enough, and deliberate enough to treat as learning
material.

## Selection Bar

The repos below made the list because they clear most or all of this bar:

- mature and actively maintained
- clearly used in real systems
- documented from the caller's point of view
- typestate used to simplify a real API, not to show off type tricks
- enough scale to show how the design behaves under pressure
- no obvious "volatile until 1.0" or "this is mostly experimental" signal

## Repositories

### rustls

Repo: <https://github.com/rustls/rustls>  
Docs: <https://docs.rs/rustls/latest/rustls/struct.ConfigBuilder.html>

### Why It Belongs

`rustls` is the clearest serious typestate-builder exemplar in the set.

Its `ConfigBuilder<Side, State>` API is explicit about the staged choices the
caller must make, and the public docs explain those stages in domain terms
before introducing the type machinery. The builder is not clever for its own
sake. The type states exist to make the legal configuration path obvious.

### What To Read

- `rustls/src/builder.rs`
- `rustls/src/client/builder.rs`
- the `ConfigBuilder` docs page before the source

### What It Teaches About Quality Code

- The public documentation explains the protocol first and the implementation
  technique second.
- The happy path is short and readable.
- Generic machinery stays in one place, while side-specific behavior is pushed
  into narrower impl blocks.
- Dangerous or special-case behavior is named as such instead of being mixed
  into the ordinary path.
- The API uses compile-time restrictions for ordering and availability, then
  runtime checks for semantic consistency that the type system cannot prove.

### What It Teaches About Typestate Builders

- Model caller obligations, not internal implementation phases.
- Keep the state set small and legible.
- Use state transitions to expose the next legal choices.
- Make required decisions happen exactly once.
- Do not let the type-level machinery leak into ordinary call sites more than
  necessary.

### What Not To Cargo-Cult

Do not copy the crypto-provider details or the exact state names. Copy the
shape: caller-facing docs, a small staged API, and a hard split between safe
defaults and explicit escape hatches.

### config-rs

Repo: <https://github.com/rust-cli/config-rs>  
Docs: <https://docs.rs/config/latest/config/builder/struct.ConfigBuilder.html>

### Why It Belongs

`config-rs` is a smaller, lighter-weight typestate builder than `rustls`. That
is part of its value. It shows a restrained use of typestate in a mature crate,
without as much domain-specific machinery around it.

Its `ConfigBuilder<St>` mainly distinguishes synchronous and asynchronous
capability. It shows that typestate is also useful for capability shifts that
change which operations are legal.

### What To Read

- `src/builder.rs`
- the `ConfigBuilder` docs page

### What It Teaches About Quality Code

- Shared behavior stays in shared impl blocks rather than being duplicated
  across states.
- The state distinction is narrow and justified.
- The docs explain when I/O actually happens and when configuration is only
  being accumulated.
- The design stays proportional. The crate does not invent more type states than
  the API needs.

### What It Teaches About Typestate Builders

- Use typestate where capabilities genuinely differ.
- Keep the common API surface common.
- A builder can change state when the caller opts into a richer capability set.
- The state parameter should remove confusion, not create it.

### What Not To Cargo-Cult

Do not read this as evidence that every builder needs type parameters. The main
lesson here is restraint.

### atsamd-hal

Repo: <https://github.com/atsamd-rs/atsamd>  
GPIO docs: <https://docs.rs/atsamd-hal/latest/atsamd_hal/gpio/index.html>

### Why It Belongs

`atsamd-hal` is not the best builder example, but it is a strong typestate API
example in a serious codebase. It is worth studying if the goal is broader than
builders and includes "how do clean projects use type-level state across a large
API surface?"

The GPIO docs are unusually useful because they explain both the preferred
type-level API and the fallback value-level API. That is a quality signal. The
project is not pretending that type-level precision is free in every context.

### What To Read

- the GPIO module docs
- `hal/src/gpio/`

### What It Teaches About Quality Code

- The docs explain tradeoffs directly.
- The type-level API is preferred, but the runtime-erased escape hatch is still
  available for callers who need it.
- The design acknowledges earlier API mistakes and replaces them with a cleaner
  model rather than defending the older shape forever.
- The codebase uses helper traits and type erasure to control generic sprawl.

### What It Teaches About Typestate APIs

- Prefer a zero-cost type-level path when the invariant is stable.
- Offer a named runtime fallback when callers genuinely need flexibility.
- Keep the fallback explicit. Do not silently weaken the entire API.
- Use type information to prevent illegal operations, not to encode every detail
  in the world.

### What Not To Cargo-Cult

Do not copy embedded-specific layering or peripheral naming into unrelated
domains. The transferable lesson is the split between the preferred static path
and the explicit dynamic escape hatch.

### stm32f4xx-hal

Repo: <https://github.com/stm32-rs/stm32f4xx-hal>  
GPIO docs: <https://docs.rs/stm32f4xx-hal/latest/stm32f4xx_hal/gpio/index.html>  
`PinMode` docs:
<https://docs.rs/stm32f4xx-hal/latest/stm32f4xx_hal/gpio/trait.PinMode.html>

### Why It Belongs

`stm32f4xx-hal` is a good complementary read next to `atsamd-hal`. It shows many
of the same core ideas with different tradeoffs and a different code shape. It
is useful when the question is whether a typestate design still holds up when
spread across a broad hardware-facing surface.

### What To Read

- the GPIO module docs
- `src/gpio.rs`
- the `PinMode` trait docs

### What It Teaches About Quality Code

- The docs make the static and dynamic modes explicit.
- Marker traits and sealing are used to keep the public invariants narrow.
- The public surface teaches the user what is legal instead of relying on prose
  alone.
- The repo exposes a dynamic alternative when ownership or ergonomic pressure
  makes the static path awkward.

### What It Teaches About Typestate APIs

- Marker traits can scale well when they encode a small, stable validity rule.
- Constrained conversions are often enough; not every transition needs a complex
  machine.
- Dynamic mode can be a deliberate escape hatch instead of an accidental back
  door.

### What Not To Cargo-Cult

Do not copy the breadth of generated peripheral-specific surface unless the
domain genuinely needs it. The main lesson is how the repo protects mode
validity while still giving users a way out when runtime flexibility is needed.

## Cross-Repo Synthesis

The most important shared lesson is that high-quality typestate code does not
feel like a type-system demo.

Across these repos, the good patterns are consistent:

- Types encode stable invariants, not every transient detail.
- The public docs explain the legal construction or usage path before showing
  the machinery underneath.
- The default path is short.
- Escape hatches exist, but they are narrow, explicit, and named in a way that
  communicates cost or risk.
- Shared behavior stays shared; only truly state-specific behavior moves into
  state-specific impls.
- Runtime validation still exists for rules that the type system cannot prove.
- Domain language drives names. Generic names are used only where they genuinely
  improve reuse.

## What These Repos Suggest About Good Typestate Builders

If we compress the lessons specifically for builder design, the shape looks like
this:

- Model the sequence of required caller decisions.
- Keep the number of builder states small.
- Put methods shared by all states in shared impls.
- Introduce a new state only when the set of legal next operations really
  changes.
- Make the happy path fluent.
- Reserve runtime errors for semantic checks that cannot be expressed as method
  availability or state progression.
- Treat "dangerous", "dynamic", or "unchecked" modes as explicit opt-ins.

## What To Copy Into Our Own Code

- Write docs from the caller's point of view first.
- Name states after domain obligations, not implementation accidents.
- Separate generic builder scaffolding from state-specific behavior.
- Prefer one small explicit escape hatch over weakening the whole API.
- Keep compiler-visible states few enough that compiler errors stay readable.
- Use runtime validation for consistency checks that remain after typestate has
  done its part.

## What Not To Copy

- Do not force typestate into domains that are mostly runtime-authored or highly
  dynamic.
- Do not create extra state types just because the type system can represent
  them.
- Do not hide a weak or confusing runtime model behind a complicated generic
  surface.
- Do not let internal phases leak into the public API unless callers truly need
  to reason about them.

## Recommended Reading Order

If the goal is "learn the most per minute," use this order:

1. `rustls` `ConfigBuilder` docs
2. `rustls/src/builder.rs`
3. `rustls/src/client/builder.rs`
4. `config-rs` `ConfigBuilder` docs and `src/builder.rs`
5. `atsamd-hal` GPIO docs
6. `stm32f4xx-hal` GPIO docs

That sequence starts with the best builder-specific exemplar, then adds a
smaller supporting builder, then broadens into large typestate-heavy APIs.

## Bottom Line

If only one external repo gets sustained attention, it should be `rustls`.

If we want a second builder example that is easier to read end to end, use
`config-rs`.

If we want to study how a mature codebase uses type-level state beyond builders,
add `atsamd-hal` and `stm32f4xx-hal`.
