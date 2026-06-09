# A Doctrine of Semantic Code

Yes. I think what you’re describing is not just “clean code” or “good architecture.” It is closer to a doctrine of **semantic fidelity**: code should preserve the shape, constraints, and language of the domain as it moves from thought → abstraction → type/module/API → call site.

Good code is not merely code that works.

Good code is code whose structure, names, types, modules, and ergonomics truthfully represent the thing being built.

The highest aim is not brevity, cleverness, reuse, or even elegance in isolation. The highest aim is **faithful intelligibility**: a human reader should be able to understand what the system means, what it permits, what it forbids, and how its pieces relate, with as little hidden context as possible.

## 1. Correctness: Code Should Model Reality Faithfully

Correctness is more than passing tests.

Correctness means the abstraction in code accurately represents the concept it claims to model.

A correct system does not merely produce the right outputs under expected conditions. It encodes the right distinctions, constraints, relationships, and impossibilities.

If the domain has two kinds of users, the code should not collapse them into an ambiguous `User`.

If the domain has an eligibility rule, the code should not scatter that rule across conditionals and helper functions.

If a value must be non-empty, validated, bounded, normalized, or semantically meaningful, it should not be represented as a naked `String` unless that lack of meaning is intentional.

Correct code makes invalid states difficult or impossible to represent.

The goal is not “types for types’ sake.” The goal is for the code to say what is true.

```rust
domain::admin::User
domain::customer::User
eligibility_policy::Play
insurance::member::Id
insurance::Member
appointment::ScheduledAt
```

These are not decorative names. They are claims about the world.

A system is more correct when the structure of the code mirrors the structure of the domain.

---

## 2. Semantic Precision Over Generic Convenience

A name should preserve the meaning needed at the point of use.

The best name is not always the shortest name. It is the shortest name that retains the necessary semantic context.

At a call site, `User` may be too vague. `AdminUser` may be an awkward compression that flattens the module structure into a prefix. But `domain::admin::User` tells the reader:

- this is a domain concept,
- it belongs to the admin subdomain,
- it is a user within that context,
- and there may be other `User` concepts elsewhere.

That is meaningful.

The module path is part of the name.

The module hierarchy should carry semantic information. The call site should use the smallest qualified path that preserves the meaning the reader needs.

Sometimes that means a fully situated path:

```rust
insurance::member::Id
insurance::member::Name
insurance::member::Member
```

And sometimes the parent module should intentionally re-export the primary semantic entity:

```rust
pub mod member;
pub use member::Member;
```

so call sites can say:

```rust
insurance::Member
```

while still preserving more specific concepts as:

```rust
insurance::member::Id
insurance::member::Name
insurance::member::EligibilityStatus
```

This is the distinction: the parent module may re-export the central entity when that improves call-site clarity, but the nested module remains the home for the vocabulary that is uniquely about that entity.

Bad code often hides meaning for convenience:

```rust
let user: User = get_user(id);
```

Better code preserves semantic context:

```rust
let user: domain::admin::User = db::admin::user.get(id)?;
```

or, depending on the domain shape:

```rust
let user: domain::admin::User = db::user::admin.get(id)?;
```

The important point is that the thing being called should itself be named with semantic context. `admin_users.get(id)` is better than `get_user(id)`, but it may still compress or lose structure. A repository/table/query handle should speak the domain through its path just as the entity does.

This is not verbosity. This is precision.

A good codebase uses paths the way natural language uses context.

---

## 3. Modularity: Separate Concepts, Not Just Files

Modularity is not the act of splitting code into many modules.

Modularity is the art of drawing boundaries around concepts that can evolve independently without duplicating logic or dissolving into abstraction soup.

A module should answer:

- What concept lives here?
- What invariants does it protect?
- What vocabulary does it define?
- What is allowed to know about its internals?
- What would change together if the domain changed?

Good modularity supports growth. It allows the system to include new things or leave things out without repeated surgery.

But modularity fails in two directions.

### Failure 1: Under-abstraction

Everything is concrete, duplicated, and local.

Rules appear in many places. Domain concepts are represented by primitive values. Changes require searching the whole codebase.

```rust
if user.role == "admin" && user.active && user.permissions.contains("billing") {
    // ...
}
```

The system “works,” but the domain is smeared everywhere.

### Failure 2: Over-abstraction

Everything is generic, indirect, parameterized, and impossible to understand.

The code avoids repetition at the cost of meaning.

```rust
EntityProcessor<TContext, TStrategy, TKind, TAdapter>
```

This may be reusable, but reusable for what? The abstraction has become detached from the domain.

Good modularity lives between these extremes.

It removes duplication without removing meaning.

---

## 4. Intelligibility: Code Should Teach the Reader

Intelligibility is how well code can be understood by someone with little or no prior context.

Readable code can be scanned.

Intelligible code can be understood.

A codebase is intelligible when its structure teaches the reader what matters.

A new reader should be able to infer:

- what the central domain concepts are,
- which concepts are distinct,
- where important rules live,
- which operations are valid,
- what the system refuses to represent,
- and what vocabulary the domain uses.

Intelligibility depends on semantic placement.

For example:

```rust
eligibility_policy::Play
```

is more intelligible than:

```rust
PlayEligibility
```

if `eligibility_policy` is a real conceptual home. The module tells the reader: this type participates in a policy system. The type name tells the reader: this is the play decision or play permission inside that policy.

The name and path cooperate.

The same principle applies across a codebase:

```rust
billing::invoice::Status
appointment::scheduling::Conflict
domain::customer::User
domain::admin::User
insurance::member::Id
insurance::Member
```

These names let the reader build a mental map.

The goal is not to make every name long. The goal is to make every name situated.

---

## 5. Readability: Code Should Be Visually and Locally Clear

Readability is the immediate physical ease of reading code.

It is affected by formatting, line length, naming, nesting, punctuation, and local flow.

Readable code avoids needless visual friction.

But readability should not be confused with minimalism.

This may be short:

```rust
let u = get(id);
```

But it is not readable in the meaningful sense, because the reader has to chase context.

This may be longer:

```rust
let admin_user = db::admin::user.get(admin_user_id)?;
```

But it is easier to read because the meaning is present.

Readability is not about reducing characters. It is about reducing uncertainty.

Good readable code has:

- clear names,
- stable patterns,
- shallow control flow,
- meaningful type signatures,
- explicit transformations,
- and call sites that reveal intent.

A readable line is one where the reader does not have to ask, “What kind of thing is this really?”

---

## 6. Explicitness: Make the Important Things Visible

Explicitness is closely tied to correctness.

If something matters to the domain, it should usually be visible in the code.

Important distinctions should not be hidden in comments, conventions, runtime checks, or tribal knowledge.

Prefer:

```rust
insurance::member::Id
provider::Id
appointment::Id
```

or concrete newtypes such as:

```rust
struct Id(String);
```

inside the semantic module that gives the name its meaning.

Prefer that over:

```rust
type Id = String;
```

or worse:

```rust
String
```

Prefer:

```rust
appointment::Request::builder()
    .member(member_id)
    .provider(provider_id)
    .time_slot(slot)
    .build()
```

over:

```rust
create_request(id1, id2, time, true, None)
```

Explicitness makes code honest.

Implicitness is acceptable for incidental details. It is dangerous for domain meaning.

A useful test:

> If misunderstanding this thing could cause a real bug, encode it explicitly.

---

## 7. Ergonomics: Correct Code Should Be Pleasant to Use

Ergonomics is not superficial convenience.

Ergonomics determines whether the correct abstraction will actually be used correctly.

If the “right” API is painful, people will route around it.

Good ergonomics make the correct path the easy path.

This is why builders, typestate builders, newtypes, and domain-specific constructors matter.

They let code be:

- explicit without being clumsy,
- safe without being noisy,
- precise without being exhausting,
- constrained without being hostile.

A builder is useful when construction has many meaningful fields:

```rust
appointment::Request::builder()
    .member(member)
    .provider(provider)
    .reason(reason)
    .requested_window(window)
    .build()
```

A typestate builder is useful when construction has required phases or compile-time guarantees:

```rust
form::Submission::builder()
    .with_patient(patient)
    .with_consent(consent)
    .with_answers(answers)
    .submit()
```

A newtype is useful when primitive values have domain meaning:

```rust
patient::EmailAddress
insurance::member::PolicyNumber
appointment::DurationMinutes
```

Ergonomics should not erase correctness. It should make correctness feel natural.

---

## 8. Types Should Carry Domain Obligations

Types are not just containers for data.

Types should carry obligations.

A good type tells the rest of the system:

- what this value means,
- what has already been validated,
- what operations are allowed,
- what states are impossible,
- and what assumptions are safe.

For example:

```rust
email::Address
```

should mean something different from `String`.

It should tell the reader: this has been parsed or validated as an email address.

Similarly:

```rust
appointment::Confirmed
appointment::Cancelled
appointment::Pending
```

may be better than one mutable `Appointment` with a loose status field, depending on the domain.

The strongest code does not merely check validity. It moves values into types that prove validity.

---

## 9. Avoid Helper Soup

A codebase decays when domain meaning is dissolved into generic helpers.

Helpers often begin innocently:

```rust
utils::validate_user()
helpers::format_status()
common::process()
```

But they become junk drawers for concepts that deserved homes.

If a helper encodes domain behavior, it probably belongs to the domain.

Prefer:

```rust
eligibility_policy::evaluate(member, service)
```

over:

```rust
utils::check_eligibility(member, service)
```

But also ask whether the operation is better expressed as behavior on one of the domain concepts:

```rust
let decision = member.eligibility_for(requested_service);
```

or:

```rust
let decision = requested_service.evaluate_eligibility_for(member);
```

or:

```rust
let decision = eligibility_policy.evaluate(member, requested_service);
```

The right owner depends on the domain. If eligibility is fundamentally a policy object, keep it on the policy. If it is a stable capability of `Member`, put it on `member`. If it belongs to the requested service, put it there. The doctrine is not “always use a free function” or “always use a method.” The doctrine is: put behavior where the semantic ownership is most truthful and where the call site best teaches the reader what is happening.

Prefer:

```rust
appointment::scheduling::detect_conflict(existing, requested)
```

over:

```rust
helpers::has_overlap(existing, requested)
```

Generic helpers are fine for genuinely generic operations. But domain logic should live in semantic modules or on the domain type that truthfully owns the behavior.

A doctrine of semantic code resists `utils`.

---

## 10. Abstraction Should Preserve Meaning

Abstraction is not inherently good.

An abstraction is good only when it captures a real shared concept.

Bad abstraction removes duplication by erasing distinctions.

Good abstraction removes duplication by naming the deeper thing that was duplicated.

If two pieces of code look similar but mean different things, do not abstract them yet.

If two pieces of code are different on the surface but obey the same domain rule, abstraction may reveal truth.

The question is not:

> Can these be made generic?

The question is:

> What concept do these share?

If there is no clear answer, duplication may be better than incoherence.

Duplication is a cost. False abstraction is debt.

---

## 11. The Call Site Is the Test

A design is not finished when the types compile.

A design must be judged at the call site.

The call site reveals whether the abstraction communicates.

Good call sites read like domain operations:

```rust
let decision = eligibility_policy.evaluate(member, requested_service);

if decision.allows(eligibility_policy::Play::ScheduleAppointment) {
    scheduler.schedule(member, requested_service)?;
}
```

or, if the domain ownership is more truthful this way:

```rust
let decision = member.eligibility_for(requested_service);

if decision.allows(eligibility_policy::Play::ScheduleAppointment) {
    scheduler.schedule(member, requested_service)?;
}
```

or:

```rust
let decision = requested_service.evaluate_eligibility_for(member);

if decision.allows(eligibility_policy::Play::ScheduleAppointment) {
    scheduler.schedule(member, requested_service)?;
}
```

The choice matters. A method is not automatically better than a module-level function. A module-level function is not automatically more explicit than a method. The best form is the one that places behavior with the concept that actually owns the invariant and makes the call site most intelligible.

Bad call sites require the reader to know too much:

```rust
let result = evaluate(user, item, true);

if result.can_do("schedule") {
    run(user, item)?;
}
```

The implementation may be clever, but the call site is where humans encounter the design.

If the call site is ambiguous, the abstraction has failed some part of its job.

---

## 12. Local Clarity Over Global Cleverness

A system should not require the reader to hold the entire architecture in their head to understand a single operation.

Local code should carry enough information to be understood locally.

This is why semantic paths matter.

This is why explicit types matter.

This is why deeply magical frameworks, hidden global state, implicit conversions, and overly clever traits can become dangerous.

A reader should not have to solve a puzzle to know what is happening.

Good code rewards inspection. Bad code requires archaeology.

---

## 13. Prefer Domain Language Over Technical Language

Technical names are sometimes necessary, but domain names should dominate domain code.

If the business/domain concept is “member eligibility,” call it that.

Do not prematurely translate it into generic technical machinery:

```rust
RuleEngineSubject
AuthorizationTarget
DynamicEntityContext
```

unless those are truly the concepts.

The code should preserve the language of the problem.

Technical architecture should support the domain, not replace it.

---

## 14. Make Illegal States Unrepresentable, but Not at the Cost of Incoherence

Strong types are valuable because they can encode truth.

But type systems can also be abused.

A type-level guarantee is worthwhile when it clarifies the domain or prevents meaningful bugs.

It is not worthwhile when it turns ordinary code into a maze of generic parameters, phantom states, trait bounds, and unreadable compiler errors without enough benefit.

Typestate is powerful when the domain has real phases:

```rust
Draft
Validated
Submitted
Accepted
Rejected
```

It is excessive when it models incidental sequencing that humans do not think about.

The doctrine is not “encode everything in the type system.”

The doctrine is:

> Encode the domain truths that matter.

---

## 15. The Best Code Feels Inevitable

When correctness, modularity, intelligibility, readability, explicitness, and ergonomics align, the code feels inevitable.

The names seem obvious.

The modules seem natural.

The call sites explain themselves.

The invalid paths are hard to take.

The correct path is easy.

The system does not feel clever. It feels shaped.

That is the goal.

---

## 16. Semantic Pressure: Awkwardness Is Information

When code feels awkward, do not immediately smooth it over.

Awkwardness may be **semantic pressure**: a signal that the domain contains an unnamed concept, a missing type, a wrong boundary, a misplaced behavior, or an invalid abstraction.

This kind of discomfort should be investigated before it is hidden.

For example:

```rust
schedule(member_id, provider_id, service_id, true, false)
```

The problem is not merely that the function is ugly. The ugliness may be telling the truth that the code has failed to name the concepts involved:

```rust
appointment::Request
insurance::member::Id
provider::Id
service::Requested
scheduling::ConflictPolicy
```

Do not use vague helpers, boolean flags, tuple arguments, generic wrappers, or premature aliases to silence semantic pressure.

First ask:

- What concept is trying to appear here?
- What invariant is being passed around without a name?
- What distinction is currently encoded only by argument position, boolean value, or convention?
- What module or type would make this feel inevitable?

Sometimes the right answer is a new type. Sometimes it is a new module. Sometimes it is moving behavior to its semantic owner. Sometimes it is accepting a small amount of duplication until the true abstraction reveals itself.

The doctrine treats awkwardness as diagnostic evidence.

---

## 17. Semantic Compression: Shorten Only After Meaning Survives

Good code compresses meaning without erasing it.

Bad compression makes code shorter by discarding semantic context:

```rust
AdminUser
MemberId
process()
repo.get(id)
```

Good compression keeps the domain structure intact while making the call site pleasant:

```rust
insurance::Member
insurance::member::Id
db::admin::user.get(id)
eligibility_policy::Play
```

The doctrine is not “make everything long.” It is “compress only after the semantic structure is preserved.”

A parent-module re-export is a valid compression when the parent module remains semantically honest:

```rust
pub mod member;
pub use member::Member;
```

Then:

```rust
insurance::Member
```

is not a loss of meaning. It is a useful compression because `insurance` still carries the domain context and `member` remains the home of member-specific vocabulary:

```rust
insurance::member::Id
insurance::member::Name
insurance::member::PolicyNumber
```

Compression becomes dangerous when it flattens or hides distinctions the reader needs.

---

## 18. Domain Core and Boundary Code

Not every part of a system deserves the same level of purity.

The **domain core** should follow this doctrine strictly. It should speak the language of the domain, encode important invariants, and resist primitive obsession, helper soup, vague services, and accidental abstractions.

**Boundary code** may speak the language of the outside world.

Boundary code includes:

- HTTP request and response DTOs,
- database rows,
- external API payloads,
- CLI arguments,
- config files,
- migrations,
- serialization formats,
- framework glue,
- generated code,
- and compatibility adapters.

Boundary code is allowed to be somewhat ugly because the outside world is often ugly.

But that ugliness should be quarantined.

The boundary should translate into semantic domain types as soon as possible:

```rust
api::CreateAppointmentRequest
```

should become:

```rust
appointment::Request
insurance::member::Id
provider::Id
appointment::RequestedWindow
```

The rule is:

> Boundary code may speak the language of the outside world. Core code should speak the language of the domain.

Do not let external naming, database convenience, JSON shapes, framework defaults, or vendor APIs infect the domain core unless they are genuinely part of the domain language.

---

## 19. Conversion Is a Semantic Act

Conversions are where truth enters or leaves the system.

Going from:

```rust
String
```

to:

```rust
email::Address
```

is not merely parsing. It is semantic promotion.

Going from:

```rust
api::MemberId
```

to:

```rust
insurance::member::Id
```

is a boundary crossing.

Therefore conversions should be explicit, named, and located at meaningful seams.

Avoid hiding meaningful domain transformations behind casual `.into()` chains when the transformation carries semantic weight:

```rust
let id = raw_id.into();
```

Prefer:

```rust
let id = insurance::member::Id::parse(raw_id)?;
```

or:

```rust
let id = insurance::member::Id::try_from_external(raw_id)?;
```

or another constructor whose name says what truth is being established.

Implicit conversions are acceptable for incidental mechanical transformations. They are dangerous when they hide validation, normalization, trust-boundary crossing, loss of precision, unit conversion, permission changes, or domain promotion.

A conversion should answer:

- What is being proven?
- What can fail?
- What boundary is being crossed?
- What meaning is gained or lost?

---

## 20. Tests Should Assert Semantics

Tests are part of the codebase’s language.

A test should usually say what domain truth it protects, not merely what mechanism it exercises.

Bad test names describe implementation trivia:

```rust
test_get_user_returns_200
test_process_valid_input
test_handler_success
```

Better test names preserve domain meaning:

```rust
admin_user_can_schedule_billing_review
customer_user_cannot_access_admin_billing_queue
expired_policy_blocks_appointment_scheduling
member_without_consent_cannot_submit_intake_form
```

Good tests become an executable glossary of domain truths.

They should make clear:

- what rule is being protected,
- what state matters,
- what behavior is allowed,
- what behavior is forbidden,
- and what invariant would be broken if the test failed.

Do not overfit tests to incidental implementation structure. Test the semantic contract. Let implementation details change when the meaning stays the same.

---

## 21. Exceptions Must Be Local, Intentional, and Quarantined

A strong doctrine needs explicit escape hatches so “pragmatism” does not become entropy.

It can be acceptable to violate the doctrine in limited cases:

- throwaway spikes,
- generated code,
- external API compatibility,
- temporary migrations,
- narrow adapters,
- serialization glue,
- performance-critical internals where profiling proves the need,
- framework integration points,
- and code whose only purpose is translation.

But an exception should be local, intentional, and explainable.

If a name is vague because it mirrors a vendor field, keep that vagueness at the boundary.

If a primitive appears because a framework requires it, convert it into a semantic type before it enters the core.

If a performance optimization damages intelligibility, isolate it behind a semantic API.

If a migration requires temporary duplication or awkwardness, mark it as transitional and prevent it from becoming the new design center.

The doctrine does not forbid compromise. It forbids unexamined compromise from spreading.

---

## 22. Invariants Should Be Encoded as Semantic Enums

Any place where meaningful invariants exist, those invariants should be encoded explicitly in the domain model.

The preferred shape is often a semantic `enum`.

An invariant is not merely a validation check. It is a truth about the domain: which states exist, which transitions are allowed, which combinations are impossible, and which distinctions matter.

If the domain has a closed set of meaningful states, phases, decisions, reasons, capabilities, statuses, outcomes, or modes, encode that set as an enum rather than as strings, booleans, numeric codes, flags, loose constants, or scattered conditionals.

Bad:

```rust
if member.status == "active" && !member.expired && member.consent_signed {
    // ...
}
```

Better:

```rust
insurance::member::Status::Active
insurance::member::Consent::Signed
insurance::policy::Coverage::Current
```

Better still, when the combination is itself the domain concept:

```rust
insurance::member::Eligibility::Eligible {
    member: insurance::member::Id,
    policy: insurance::policy::Id,
}

insurance::member::Eligibility::Ineligible {
    member: insurance::member::Id,
    reason: insurance::member::eligibility::DenialReason,
}
```

The enum is the semantic center. The surrounding patterns should be refactored around it.

That means builders, typestate builders, newtypes, smart constructors, validation functions, errors, and module boundaries should serve the invariant-bearing enum rather than hiding or duplicating the invariant elsewhere.

For example, if appointment submission has phases, do not merely encode that as builder mechanics or boolean fields:

```rust
struct Submission {
    validated: bool,
    submitted: bool,
}
```

Prefer explicit semantic states:

```rust
intake::submission::State::Draft
intake::submission::State::Validated
intake::submission::State::Submitted
intake::submission::State::Accepted
intake::submission::State::Rejected
```

Then design the typestate builder, transitions, errors, and APIs around those states:

```rust
intake::Submission<intake::submission::Draft>
intake::Submission<intake::submission::Validated>
intake::submission::Error::CannotSubmitDraft
```

or, where a runtime enum better matches the domain:

```rust
intake::submission::Submission {
    state: intake::submission::State,
}
```

The specific Rust pattern depends on the domain. Some invariants want a plain enum. Some want a newtype plus enum. Some want typestate with zero-sized marker types. Some want a builder that can only produce a valid enum variant. Some want `nutype`-validated fields inside enum variants.

But the doctrine is explicit:

> Find the invariant. Name it. Encode it as a semantic domain concept. Then organize the ergonomics around that concept.

Do not let invariants live primarily in:

- comments,
- docs,
- tests only,
- booleans,
- strings,
- magic numbers,
- validation helpers,
- database constraints only,
- frontend-only checks,
- builder step mechanics,
- or scattered `if` statements.

Those may support the invariant, but they should not be its primary home.

The primary home should be the domain model.

---

## 23. Errors Are Semantic Domain Values

Errors should not be generic afterthoughts.

An error is a domain value. It says what failed, where it failed, what invariant was violated, what context matters, and what kind of recovery may be possible.

Therefore errors should follow the same doctrine of semantic fidelity as successful values.

Prefer each module that has meaningful failure modes to define its own `error.rs`:

```text
insurance/
  mod.rs
  error.rs
  member/
    mod.rs
    error.rs
```

The module’s `error.rs` should define the module’s semantic error type and result alias:

```rust
#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum Error {
    #[display("member not found: {id}")]
    MemberNotFound { id: member::Id },

    #[display("member is not eligible for {service}")]
    MemberNotEligible {
        member: member::Id,
        service: service::Id,
    },
}

pub type Result<T> = core::result::Result<T, Error>;
```

This allows call sites to preserve semantic context:

```rust
fn load_member(id: insurance::member::Id) -> insurance::Result<insurance::Member> {
    // ...
}
```

or, for a nested module:

```rust
fn parse_id(raw: &str) -> insurance::member::Result<insurance::member::Id> {
    // ...
}
```

Parent modules should compose child-module errors explicitly. With `derive_more`, parent errors should use `#[from]` where conversion is semantically honest:

```rust
#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
pub enum Error {
    #[display("member error")]
    Member(#[from] member::Error),

    #[display("policy error")]
    Policy(#[from] policy::Error),
}

pub type Result<T> = core::result::Result<T, Error>;
```

The point is not merely convenience for `?`. The point is that error propagation preserves the semantic path of the failure.

A child error should remain a child error when lifted into the parent. Do not flatten it into a vague message, erase its typed context, or convert it into `anyhow::Error` in the domain core.

Good:

```rust
insurance::member::Error::InvalidId { raw }
insurance::Error::Member(member::Error::InvalidId { raw })
appointment::scheduling::Error::ConflictingAppointment { existing, requested }
```

Bad:

```rust
Error::InvalidInput
Error::Failed
anyhow!("something went wrong")
String
```

### `derive_more` and SNAFU

The default preference is explicit module-local errors with:

```rust
pub type Result<T> = core::result::Result<T, Error>;
```

and parent composition via `#[from]` using `derive_more`, when this is sufficient.

SNAFU is also philosophically compatible with this doctrine when its context selectors make failures more explicit and contextual. SNAFU’s strength is that it encourages adding context at the failure site, especially when the same underlying error can occur in different semantic contexts.

That philosophy is good: the same low-level error may mean different things depending on what the domain was trying to do.

For example, an `std::io::Error` is not semantically complete by itself. It matters whether the system was reading an eligibility rules file, writing an audit record, loading a member import, or opening a migration checkpoint.

With SNAFU-style context, the failure should become domain-specific:

```rust
#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("could not read eligibility rules from {path}"))]
    ReadEligibilityRules {
        source: std::io::Error,
        path: camino::Utf8PathBuf,
    },
}

pub type Result<T> = core::result::Result<T, Error>;
```

Use SNAFU when its context machinery improves semantic precision and ergonomics. Use `derive_more` when straightforward explicit enum composition is clearer. Do not choose an error crate because it is fashionable. Choose the approach that best preserves the meaning of the failure at the call site and across module boundaries.

### Error Doctrine Rules

- Every semantically meaningful module with meaningful failure modes should have an `error.rs`.
- Each such module should expose `pub type Result<T> = core::result::Result<T, Error>;`.
- Error variants should be named after domain failures, not implementation accidents.
- Error variants should carry typed semantic context, not unstructured strings.
- Parent modules should compose child errors explicitly, usually via `#[from]`.
- Use low-level source errors as sources, not as replacements for domain errors.
- Avoid `anyhow`, `Box<dyn Error>`, `String`, or vague catch-all errors in the domain core.
- Boundary/application layers may erase or report errors, but the core should preserve semantic error structure.
- Error messages should be human-readable, but the enum structure should be machine-meaningful.
- `?` should propagate meaning, not discard it.

---

# Practical Rules

## Naming

Use semantic module paths.

Prefer:

```rust
insurance::member::Id
insurance::member::Name
insurance::member::Member
insurance::Member // if `insurance` intentionally re-exports `member::Member`
domain::admin::User
domain::customer::User
```

over:

```rust
Id
Name
User
AdminUser
CustomerUser
MemberId
```

when the module path carries important meaning.

Avoid flattening semantic hierarchy into awkward prefixes unless the type must frequently exist outside its module context and the prefix is genuinely clearer than the path.

The module path is part of the name. Use it deliberately.

A good module can expose both nested semantic vocabulary and parent-level ergonomic re-exports:

```rust
pub mod member;
pub use member::Member;
```

Then use:

```rust
insurance::Member
insurance::member::Id
insurance::member::Name
```

This preserves both ergonomics and semantic fidelity.

---

## Modules

Modules should be organized around domain concepts, not technical categories alone.

Prefer:

```text
insurance/
  mod.rs              // pub use member::Member;
  member/
    mod.rs            // Member, Id, Name, PolicyNumber, EligibilityStatus
    eligibility.rs
  claim/
  policy/

domain/
  admin/
    user.rs
  customer/
    user.rs

eligibility_policy/
  play.rs
  decision.rs
  evaluator.rs

appointment/
  scheduling/
  cancellation/
  confirmation/
```

over:

```text
models/
services/
utils/
helpers/
processors/
```

Technical folders are acceptable at boundaries, but the core should speak the domain.

---

## Repositories, Tables, Stores, and Query Handles

Semantic naming applies not only to entities, but also to the thing being called.

Prefer handles whose path tells the reader what is being queried:

```rust
db::admin::user.get(id)
db::user::admin.get(id)
insurance_db::member.get(id)
appointment_store::scheduling::conflict.find(requested_slot)
```

over vague or flattened handles:

```rust
users.get(id)
admin_users.get(id)
repo.get(id)
store.find(id)
```

`admin_users.get(id)` may be acceptable locally, but it often loses context. `db::admin::user.get(id)` or `db::user::admin.get(id)` lets the data-access path carry the same semantic burden as the entity path.

Choose the order that reflects the actual domain: if “admin” is a kind of user, `db::user::admin` may be best; if “admin” is a subdomain with user records inside it, `db::admin::user` may be best. The doctrine does not demand one universal hierarchy. It demands that the hierarchy tell the truth.

---

## Types

Use semantic types for semantic values.

Prefer:

```rust
insurance::member::Id
provider::Id
appointment::Id
```

or, where the module is not present at the call site and the flattened name is truly clearer:

```rust
MemberId
ProviderId
AppointmentId
```

But default toward module-qualified semantic types when the path carries meaning.

Avoid naked primitives for values with domain meaning.

Use newtypes when:

- two values share a primitive representation but differ semantically,
- validation matters,
- accidental swapping would be dangerous,
- or the type gives the reader important context.

---

## Builders

Use builders when construction has several meaningful parts.

Use typestate builders when construction has required phases or compile-time completeness matters.

A builder should make construction clearer, not merely more ceremonious.

Good:

```rust
intake::Form::builder()
    .patient(patient)
    .answers(answers)
    .consent(consent)
    .build()
```

Bad:

```rust
ThingBuilder::new()
    .set_a(a)
    .set_b(b)
    .set_c(c)
    .execute()
```

if `a`, `b`, and `c` are still semantically vague.

---

## Abstraction

Do not abstract because code looks similar.

Abstract because concepts are the same.

Before introducing an abstraction, ask:

1. What domain concept does this represent?
2. What invariant does it protect?
3. What future variation does it allow?
4. What meaning would be lost?
5. Is the call site clearer after abstraction?

If the abstraction cannot answer those questions, wait.

---

## Explicitness

Expose meaningful domain distinctions.

Hide incidental mechanics.

A good API hides boring details and reveals important ones.

Bad explicitness exposes implementation noise.

Good explicitness exposes semantic truth.

---

## Ergonomics

The correct path should be the easy path.

If a safe abstraction is painful to use, improve its ergonomics rather than bypassing it.

Use:

- smart constructors,
- builders,
- typed IDs,
- domain-specific methods,
- module-qualified names,
- parent-module re-exports for central entities,
- narrow traits,
- validation types,
- and compile-time states

 to make correct usage natural.

---

# Short Version

Good code should be:

- **Correct**: it faithfully models the domain.
- **Modular**: it separates real concepts without creating abstraction fog.
- **Intelligible**: it teaches a new reader what the system means.
- **Readable**: it reduces local uncertainty and visual friction.
- **Explicit**: it makes important distinctions visible.
- **Ergonomic**: it makes the correct thing easy to do.
- **Semantic**: its names, modules, and types preserve meaning.
- **Honest**: it does not hide domain complexity behind vague helpers or generic machinery.

The doctrine might be summarized as:

> Code should preserve meaning.  
> The structure of the program should reflect the structure of the domain.  
> The type system should encode important truths.  
> The module path should carry semantic context.  
> The call site should be understandable without archaeology.  
> Abstraction should clarify, not obscure.  
> Ergonomics should make correctness natural.

Or even shorter:

> Write code whose shape tells the truth.
