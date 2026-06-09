# Semantic Code Review Checklist

Use this before accepting any substantial coding change.

## Domain Model

- [ ] Domain concepts are represented faithfully.
- [ ] Module paths carry semantic meaning.
- [ ] Parent re-exports compress meaning without erasing context.
- [ ] Call sites preserve the smallest qualified path needed for understanding.

## Invariants

- [ ] Meaningful invariants are identified explicitly.
- [ ] Closed sets of states/phases/decisions/reasons/capabilities/statuses/outcomes/modes are encoded as semantic enums or enum-centered concepts.
- [ ] Invariants are not primarily hidden in strings, booleans, flags, magic numbers, validation helpers, database constraints, frontend-only checks, builder mechanics, or scattered conditionals.
- [ ] Builders/typestate builders/newtypes/nutypes/smart constructors are organized around invariant-bearing concepts.

## Types and Conversions

- [ ] Naked primitives are avoided for meaningful values.
- [ ] Typed IDs and domain values use semantic modules, e.g. `insurance::member::Id`.
- [ ] Meaningful conversions are explicit and named.
- [ ] Boundary-to-domain promotion is visible and fallible when appropriate.
- [ ] Casual `.into()` is not hiding validation, normalization, trust-boundary crossing, unit conversion, permission change, or domain promotion.

## Errors

- [ ] Modules with meaningful failure modes have `error.rs`.
- [ ] Each such module defines `Error` and `pub type Result<T> = core::result::Result<T, Error>;`.
- [ ] Error variants name domain failures, not implementation accidents.
- [ ] Error variants carry typed semantic context.
- [ ] Parent errors compose child errors explicitly, usually with `#[from]`.
- [ ] Domain core avoids `anyhow`, `Box<dyn Error>`, `String`, `Error::Failed`, and vague catch-alls.
- [ ] SNAFU is used only where context selectors improve semantic precision and ergonomics.

## Boundaries

- [ ] Boundary ugliness is quarantined in DTOs/adapters/framework glue.
- [ ] External API/DB/config/framework names do not leak into the domain core unless genuinely domain language.
- [ ] Domain code speaks domain language.

## Abstraction and Modularity

- [ ] Abstractions name real shared concepts.
- [ ] Similar-looking but semantically different code is not prematurely abstracted.
- [ ] Helper soup is avoided; domain behavior lives in semantic modules or on the semantic owner.
- [ ] Semantic pressure is investigated rather than hidden behind convenience wrappers.

## Tests

- [ ] Tests assert domain truths.
- [ ] Test names are an executable glossary, not implementation trivia.
- [ ] Tests protect important invariants and transitions.
- [ ] Tests are not overfit to incidental implementation structure.

## Exceptions

- [ ] Any doctrine violation is local, intentional, and explainable.
- [ ] Exceptions are quarantined behind semantic APIs.
- [ ] Transitional compromises are marked and prevented from becoming the design center.
