# Semantic Code Task Prompt Template

Use this when asking Hermes, Codex, Claude Code, OpenCode, or another coding agent to implement/refactor/review code under the doctrine.

```text
Load and follow the `semantic-code-doctrine` skill. Treat it as a first-class design constraint.

Task:
<describe the feature/refactor/bug/review>

Repository/path:
<path>

Doctrine requirements:
- Preserve semantic fidelity: code shape should reflect domain shape.
- Identify domain concepts, invariants, boundaries, and failure modes before editing.
- Encode meaningful invariants as semantic enums or enum-centered domain concepts.
- Refactor builders, typestate builders, newtypes/nutypes, smart constructors, errors, and module boundaries around invariant-bearing concepts.
- Prefer semantic module paths and parent re-exports where they improve call-site clarity.
- Avoid primitive obsession, boolean/string state, helper soup, vague services, and false abstractions.
- Add module-local `error.rs` files with `Error` and `pub type Result<T> = core::result::Result<T, Error>;` where modules have meaningful failure modes.
- Compose parent errors from child errors explicitly, usually with `#[from]` via `derive_more`, or use SNAFU if context selectors make errors more semantically precise.
- Keep boundary ugliness quarantined; convert external DTO/API/DB/config/framework shapes into semantic domain types at the seam.
- Make meaningful conversions explicit and named; avoid casual `.into()` for semantic promotion or trust-boundary crossing.
- Tests should assert domain truths and be named accordingly.

Execution protocol:
1. Inspect the relevant code before planning.
2. Produce a short semantic model: concepts, invariants, boundaries, errors, tests.
3. Implement in small coherent edits.
4. Run the appropriate tests/lints.
5. Review changed call sites against the doctrine.
6. Report what changed, what invariants are now encoded, what errors/results were introduced, what tests prove, and any quarantined exceptions.
```
