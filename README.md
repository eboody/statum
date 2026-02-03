<div align="center">
    <img src="https://github.com/eboody/statum/raw/main/logo.png" alt="statum Logo" width="150">
</div>

# Statum

**Statum** is a zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation.

### Why Use Statum?
- **Compile-Time Safety**: State transitions are validated at compile time, ensuring no invalid transitions.
- **Ergonomic Macros**: Define states and state machines with minimal boilerplate.
- **State-Specific Data**: Add and access data tied to individual states easily.
- **Persistence-Friendly**: Reconstruct state machines seamlessly from external data sources.


## Table of Contents
- [Quick Start](#quick-start)
- [Additional Features & Examples](#additional-features--examples)
  - [Adding `Debug`, `Clone`, or Other Derives](#1-adding-debug-clone-or-other-derives)
  - [Complex Transitions & Data-Bearing States](#2-complex-transitions--data-bearing-states)
  - [Reconstructing State Machines from Persistent Data](#3-reconstructing-state-machines-from-persistent-data)
  - [Typestate Builder Ergonomics](#4-typestate-builder-ergonomics)
- [Examples](#examples)
- [Patterns & Guidance](#patterns--guidance)
- [API Rules (Current)](#api-rules-current)
- [Common Errors and Tips](#common-errors-and-tips)
- [API Reference](#api-reference)

## Quick Start

To start, it provides three attribute macros:

- **`#[state]`** for defining states (as enums).
- **`#[machine]`** for creating a state machine struct that tracks which state you are in at compile time.
- **`#[transition]`** for validating transition method signatures.

Here is the simplest usage of Statum without any extra features:

```rust
use statum::{machine, state, transition};

// 1. Define your states as an enum.
#[state]
pub enum LightState {
    Off,
    On,
}

// 2. Define your machine with the #[machine] attribute.
#[machine]
pub struct LightSwitch<LightState> {
    name: String, // Contextual, machine-wide fields go here.
}

// 3. Implement transitions for each state.
#[transition]
impl LightSwitch<Off> {
    pub fn switch_on(self) -> LightSwitch<On> {
        self.transition()
    }
}

#[transition]
impl LightSwitch<On> {
    pub fn switch_off(self) -> LightSwitch<Off> {
        self.transition()
    }
}

fn main() {
    // 4. Create a machine with the "Off" state.
    let light = LightSwitch::<Off>::builder()
        .name("desk lamp".to_owned())
        .build();

    // 5. Transition from Off -> On, On -> Off, etc.
    let light = light.switch_on(); // type: LightSwitch<On>
    let _light = light.switch_off(); // type: LightSwitch<Off>
}
```

Example: [statum-examples/src/examples/example_01_setup.rs](statum-examples/src/examples/example_01_setup.rs).

### How It Works

- `#[state]` transforms your enum, generating one struct per variant (like `Off` and `On`), plus a trait `LightState`.
- `#[machine]` injects extra fields (`marker`, `state_data`) to track which state you are in, letting you define transitions that change the state at the type level.
- `#[transition]` validates method signatures and ties them to a concrete next state.

That is it. You now have a compile-time guaranteed state machine where invalid transitions are impossible.

---

## Additional Features & Examples

### 1. Adding `Debug`, `Clone`, or Other Derives

By default, you can add normal Rust derives on your enum and struct. For example:

```rust
#[state]
#[derive(Debug, Clone)]
pub enum LightState {
    Off,
    On,
}

#[machine]
#[derive(Debug, Clone)]
pub struct LightSwitch<LightState> {
    name: String,
}
```

**Important**: If you place `#[derive(...)]` above `#[machine]`, you may see an error like:

```
error[E0063]: missing fields `marker` and `state_data` in initializer of `Light<_>`
   |
14 | #[derive(Debug, Clone)]
   |          ^ missing `marker` and `state_data`
```

**To avoid this**, put `#[machine]` above the derive(s).

```rust
// ❌ This will NOT work
#[derive(Debug)] // note the position of the derive
#[machine]
pub struct LightSwitch<LightState>;

// ✅ This will work
#[machine]
#[derive(Debug)]
pub struct LightSwitch<LightState>;
```

Example: [statum-examples/src/examples/03-derives.rs](statum-examples/src/examples/03-derives.rs).

---

### 2. Complex Transitions & Data-Bearing States

#### Defining State Data
States can hold data. For example:

```rust
#[state]
pub enum ReviewState {
    Draft,
    InReview(ReviewData), // State data
    Published,
}

pub struct ReviewData {
    reviewer: String,
    notes: Vec<String>,
}

#[machine]
pub struct Document<ReviewState> {
    id: String,
}

#[transition]
impl Document<Draft> {
    pub fn submit_for_review(self, reviewer: String) -> Document<InReview> {
        let data = ReviewData { reviewer, notes: vec![] };
        self.transition_with(data)
    }
}
```

> Note: We use `self.transition_with(data)` instead of `self.transition()` to transition to a state that carries data.

#### Accessing State Data

State data is available as `self.state_data` in the concrete machine type:

```rust
impl Document<InReview> {
    fn add_note(&mut self, note: String) {
        self.state_data.notes.push(note);
    }

    fn approve(self) -> Document<Published> {
        self.transition()
    }
}
```

Examples: [statum-examples/src/examples/07-state-data.rs](statum-examples/src/examples/07-state-data.rs), [statum-examples/src/examples/08-transition-with-data.rs](statum-examples/src/examples/08-transition-with-data.rs).

---

### 3. Reconstructing State Machines from Persistent Data

State machines often need to persist their state. Saving to and loading from external storage like databases should be both robust and type-safe. Statum's `#[validators]` macro simplifies this process, ensuring seamless integration between your persistent data and state machine logic.

The key pieces are:
- `#[validators]` macro on your data type impl block.
- `machine_builder()` generated on the data type to reconstruct the machine.

#### Example

```rust
use statum::{machine, state, validators};

#[state]
pub enum TaskState {
    Draft,
    InReview(ReviewData),
    Published,
}

pub struct ReviewData {
    reviewer: String,
}

#[machine]
struct TaskMachine<TaskState> {
    client: String,
    name: String,
    priority: u8,
}

enum Status {
    Draft,
    InReview,
    Published,
}

struct DbData {
    state: Status,
}

#[validators(TaskMachine)]
impl DbData {
    fn is_draft(&self) -> Result<(), statum::Error> {
        match self.state {
            Status::Draft => {
                // Note: machine fields are available here (client, name, priority).
                println!("Client: {}, Name: {}, Priority: {}", client, name, priority);
                Ok(())
            }
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_in_review(&self) -> Result<ReviewData, statum::Error> {
        match self.state {
            Status::InReview => Ok(ReviewData { reviewer: "sam".into() }),
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_published(&self) -> Result<(), statum::Error> {
        match self.state {
            Status::Published => Ok(()),
            _ => Err(statum::Error::InvalidState),
        }
    }
}

fn main() {
    let db_data = DbData { state: Status::InReview };

    let machine = db_data
        .machine_builder()
        .client("acme".to_owned())
        .name("doc".to_owned())
        .priority(1)
        .build()
        .unwrap();

    match machine {
        TaskMachineSuperState::Draft(_draft_machine) => { /* ... */ }
        TaskMachineSuperState::InReview(_in_review_machine) => { /* ... */ }
        TaskMachineSuperState::Published(_published_machine) => { /* ... */ }
    }
}
```

Examples: [statum-examples/src/examples/09-persistent-data.rs](statum-examples/src/examples/09-persistent-data.rs), [statum-examples/src/examples/10-persistent-data-vecs.rs](statum-examples/src/examples/10-persistent-data-vecs.rs).

## Examples

See `statum-examples/src/examples/` for the full suite of examples.

---

## Patterns & Guidance

### Conditional transitions (branching decisions)
Transition methods must return a single next state. Put branching logic in a normal method and call explicit transition methods:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (event-driven transitions).

```rust
#[transition]
impl ProcessMachine<Init> {
    fn to_next(self) -> ProcessMachine<NextState> {
        self.transition()
    }

    fn to_other(self) -> ProcessMachine<OtherState> {
        self.transition()
    }
}

enum Decision {
    Next(ProcessMachine<NextState>),
    Other(ProcessMachine<OtherState>),
}

impl ProcessMachine<Init> {
    fn decide(self, event: u8) -> Decision {
        if event == 0 {
            Decision::Next(self.to_next())
        } else {
            Decision::Other(self.to_other())
        }
    }
}
```

### Event-driven transitions
Model events as an enum and route them to explicit transition methods:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (event-driven transitions).

```rust
enum Event {
    Go,
    Alternative,
}

enum Decision {
    Next(ProcessMachine<NextState>),
    Other(ProcessMachine<OtherState>),
}

impl ProcessMachine<Init> {
    fn handle_event(self, event: Event) -> Decision {
        match event {
            Event::Go => Decision::Next(self.to_next()),
            Event::Alternative => Decision::Other(self.to_other()),
        }
    }
}
```

### Guarded transitions
Keep preconditions in a guard method and return a `Result` before transitioning:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (guarded transitions).

```rust
impl Machine<Pending> {
    fn can_activate(&self) -> bool {
        self.allowed
    }

    fn try_activate(self) -> Result<Machine<Active>, statum::Error> {
        if self.can_activate() {
            Ok(self.activate())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}
```

### Hierarchical machines (state data as a nested machine)
Use a nested machine as state data to model parent and child flows:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (hierarchical machines). Example: [statum-examples/src/examples/11-hierarchical-machines.rs](statum-examples/src/examples/11-hierarchical-machines.rs).

```rust
#[state]
enum SubState {
    Idle,
    Running,
}

#[machine]
struct SubMachine<SubState> {}

#[state]
enum ParentState {
    NotStarted,
    InProgress(SubMachine<Running>),
    Done,
}

#[machine]
struct ParentMachine<ParentState> {}
```

### State snapshots (carry previous state data forward)
Capture the prior state's data inside the next state to keep history:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (state snapshots).

```rust
#[transition]
impl Machine<Draft> {
    fn publish(self) -> Machine<Published> {
        let previous = self.state_data.clone();
        self.transition_with(PublishData { previous })
    }
}
```

### Rollbacks / undo transitions
Model rollbacks by returning to a previous state explicitly, often with stored state data:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (rollbacks). Example: [statum-examples/src/examples/12-rollbacks.rs](statum-examples/src/examples/12-rollbacks.rs).

```rust
#[transition]
impl Document<Published> {
    fn rollback(self) -> Document<Draft> {
        self.transition()
    }
}
```

### Async transitions (side effects before transition)
Keep side effects in async methods and call a sync transition at the end:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (async side-effects). Example: [statum-examples/src/examples/06-async-transitions.rs](statum-examples/src/examples/06-async-transitions.rs).

```rust
#[transition]
impl Job<Queued> {
    fn start(self) -> Job<Running> {
        self.transition()
    }
}

impl Job<Queued> {
    async fn start_with_effects(self) -> Job<Running> {
        do_io().await;
        self.start()
    }
}
```

### Rehydration with extra fetch
Use machine fields inside validators to fetch extra data for state reconstruction:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (rehydration with fetch).

```rust
#[validators(TaskMachine)]
impl DbData {
    fn is_in_review(&self) -> Result<ReviewData, statum::Error> {
        match self.state {
            Status::InReview => Ok(ReviewData { reviewer: fetch_reviewer(client) }),
            _ => Err(statum::Error::InvalidState),
        }
    }
}
```

### Persistent batches
When reconstructing many rows, use the batch builder on collections:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (parallel reconstruction, batch builder). Example: [statum-examples/src/examples/10-persistent-data-vecs.rs](statum-examples/src/examples/10-persistent-data-vecs.rs).

```rust
let results = rows
    .machines_builder()
    .client(client)
    .build();
```

If you want a plain `Result<Vec<Machine>, Error>` without skipping invalid rows, map and collect:

```rust
let machines: Result<Vec<TaskMachineSuperState>, statum::Error> = rows
    .into_iter()
    .map(|row| {
        row.machine_builder()
            .client(client.clone())
            .name(name.clone())
            .priority(priority)
            .build()
    })
    .collect();
```

### Parallel reconstruction (async validators)
If validators are async, the batch builder returns results in parallel:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (parallel reconstruction).

```rust
let results = rows
    .machines_builder()
    .tenant(tenant)
    .build()
    .await;
```

### Type-erased storage (collecting superstates)
Store `*SuperState` values in a collection and match later:

Tested in [statum-examples/tests/patterns.rs](statum-examples/tests/patterns.rs) (type-erased storage).

```rust
let items: Vec<TaskMachineSuperState> = vec![machine];
for item in items {
    match item {
        TaskMachineSuperState::Draft(m) => { /* ... */ }
        _ => {}
    }
}
```

---

## API Rules (Current)

### `#[state]`
- Must be an enum.
- Must have at least one variant.
- Variants must be unit or single-field tuple variants.
- Generics on the enum are not supported.

### `#[machine]`
- Must be a struct.
- First generic parameter must match the `#[state]` enum name.
- Derives on `#[state]` are propagated to generated variant types.
- Prefer `#[machine]` above `#[derive(..)]` to avoid derive ordering surprises.

### `#[transition]`
- Must be applied to `impl Machine<State>` blocks.
- Methods must take `self` or `mut self` as the first argument.
- Return type must be `Machine<NextState>` or `Option<Result<...>>` wrappers.
- Data-bearing states must use `transition_with(data)`.

### `#[validators]`
- Use `#[validators(Machine)]` on an `impl` block for your persistent data type.
- Must define an `is_{state}` method for every state variant (snake_case).
- Each method returns `Result<()>` for unit states or `Result<StateData>` for data states.
- Async validators are supported; if any validator is async, the generated builder is async.
- The macro generates a `{Machine}SuperState` enum for matching on reconstructed states (typestate builder pattern).

---

## Common Errors and Tips

1. **`missing fields marker and state_data`**  
   - Usually means your derive macros (e.g., `Clone` or `Debug`) expanded before Statum could inject those fields. Move `#[machine]` above your derives, or remove them.

2. **`cannot find type X in this scope`**  
   - Ensure that you define your `#[machine]` struct before you reference it in `impl` blocks or function calls.

3. **`Invalid transition return type`**  
   - Transition methods must return `Machine<NextState>` (optionally wrapped in `Option` or `Result`).

---

## API Reference

### **Core Macros**

| Macro       | Description                                                                                   | Example Usage                                                |
|-------------|-----------------------------------------------------------------------------------------------|-------------------------------------------------------------|
| `#[state]`  | Defines states as an enum. Each variant becomes its own struct implementing the `State` trait. | `#[state] pub enum LightState { Off, On }`                  |
| `#[machine]`| Defines a state machine struct and injects fields for state tracking and transitions.          | `#[machine] pub struct Light<LightState> { name: String }`  |
| `#[transition]`| Validates transition methods and generates the proper transition helpers.                   | `#[transition] impl Light<Off> { fn on(self)->Light<On>{...} }` |
| `#[validators]` | Defines validation methods to map persistent data to specific states.                      | `#[validators(TaskMachine)]`                                |

---

### **State Machine Methods / Fields**

| Item                | Description                                                                                           | Example Usage                                                |
|---------------------|-------------------------------------------------------------------------------------------------------|-------------------------------------------------------------|
| `.builder()`        | Builds a new machine in a specific state.                                                             | `LightSwitch::<Off>::builder().name("lamp").build()`       |
| `.transition()`     | Transitions to a unit state.                                                                           | `let light = light.switch_on();`                            |
| `.transition_with(data)` | Transitions to a state that carries data.                                                        | `self.transition_with(data)`                                |
| `.state_data`       | Accesses the data of the current state (if available).                                                 | `let notes = &self.state_data.notes;`                       |

---

### **Validators Output**

| Item                | Description                                                                                           | Example Usage                                                |
|---------------------|-------------------------------------------------------------------------------------------------------|-------------------------------------------------------------|
| `{Machine}SuperState` | Wrapper enum for all machine states, used for matching.                                               | `match machine { TaskMachineSuperState::Draft(m) => ... }`  |
| `machine_builder()` | Builder generated on the data type to reconstruct a machine from stored data.                         | `row.machine_builder().client(c).build()`                   |

---
