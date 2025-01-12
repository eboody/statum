<div align="center">
    <img src="https://github.com/eboody/statum/raw/main/logo.png" alt="statum Logo" width="150">
</div>

# Statum

**Statum** is a zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation. To start, it provides two attribute macros:

### Why Use Statum?
- **Compile-Time Safety**: State transitions are validated at compile time, ensuring no invalid transitions.
- **Ergonomic Macros**: Define states and state machines with minimal boilerplate.
- **State-Specific Data**: Add and access data tied to individual states easily.
- **Persistence-Friendly**: Reconstruct state machines seamlessly from external data sources.


## Table of Contents
- [Quick Start](#quick-start)
- [Additional Features & Examples](#additional-features--examples)
- [Complex Transitions & Data-Bearing States](#3-complex-transitions--data-bearing-states)
- [Serde Integration](#2-serde-integration)
- [Reconstructing State Machines from Persistent Data](#4-reconstructing-state-machines-from-persistent-data)
- [Common Errors and Tips](#common-errors-and-tips)

## Quick Start

- **`#[state]`** for defining states (as enums).
- **`#[machine]`** for creating a state machine struct that tracks which state you’re in at compile time.

There is one more super useful macro, but read on to find out more!

Here’s the simplest usage of Statum without any extra features:

```rust
use statum::{machine, state};

// 1. Define your states as an enum.
#[state]
pub enum LightState {
    Off,
    On,
}

// 2. Define your machine with the #[machine] attribute.
#[machine]
pub struct Light<S: LightState> {
    name: String, // Contextual, Machine-wide fields go here, like clients, configs, an identifier, etc.
}

// 3. Implement transitions for each state.
impl Light<Off> {
    pub fn switch_on(self) -> Light<On> {
        //Note: we consume self and return a new state
        self.transition()
    }
}

impl Light<On> {
    pub fn switch_off(self) -> Light<Off> {
        self.transition()
    }
}

fn main() {
    // 4. Create a machine with the "Off" state.
    let light = Light::new("desk lamp".to_owned());

    // 5. Transition from Off -> On, On -> Off, etc.
    let light = light.switch_on(); //is type Light<On>
    let light = light.switch_off(); // is type Light<Off>
}
```

### How It Works

- `#[state]` transforms your enum, generating one struct per variant (like `Off` and `On`), plus a trait `LightState`.
- `#[machine]` injects extra fields (`marker`, `state_data`) to track which state you’re in, letting you define transitions that change the state at the type level.

That’s it! You now have a compile-time guaranteed state machine where invalid transitions are impossible.

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
pub struct Light<S: LightState> {
    name: String,
}
```

**Important**: If you place `#[derive(...)]` _above_ `#[machine]`, you may see an error like:

```
error[E0063]: missing fields `marker` and `state_data` in initializer of `Light<_>`
   |
14 | #[derive(Debug, Clone)]
   |          ^ missing `marker` and `state_data`
```

**To avoid this**, put `#[machine]` _above_ the derive(s).

```rust
// ❌ This will NOT work
#[derive(Debug)] // ↩ note the position of the derive
#[machine]
pub struct Light<S: LightState>;

// ✅ This will work
#[machine]
#[derive(Debug)]
pub struct Light<S: LightState>;

```

---

### 2. `serde` Integration

Statum can optionally propagate `Serialize`/`Deserialize` derives if you enable the `"serde"` feature and derive those on your `#[state]` enum. For example:

```toml
[dependencies]
statum = { version = "x.y.z", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
```

Then, in your code:

```rust
#[state]
#[derive(Serialize, Deserialize)]
pub enum DocumentState {
    Draft,
    Published,
}
```

---

### 3. Complex Transitions & Data-Bearing States

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
pub struct Document<S: ReviewState> {
    id: String,
    content: String,
}

// ...

impl Document<Draft> {
    pub fn submit_for_review(self, reviewer: String) -> Document<InReview> {
        let data = ReviewData { reviewer, notes: vec![] };
        self.transition_with(data)
    }
}

// ...
```

> Note: We use `self.transition_with(data)` instead of `self.transition()` to transition to a state that carries data.

#### Accessing State Data

Use `.get_state_data()` or `.get_state_data_mut()` to interact with the state-specific data:

```rust
impl Document<Review> {
    fn add_note(&mut self, note: String) {
        if let Some(review_data) = self.get_state_data_mut() {
            review_data.notes.push(note);
        }
    }

    fn reviewer_name(&self) -> Option<&str> {
        self.get_state_data().map(|data| data.reviewer.as_str())
    }

    fn approve(self) -> Document<Published> {
        self.transition()
    }
}
```
---

### 4. Reconstructing State Machines from Persistent Data

State machines in real-world applications often need to **persist their state**—saving to and loading from external storage like databases. Reconstructing a state machine from this data must be both robust and type-safe. Statum's `#[validators]` macro simplifies this process, ensuring seamless integration between your persistent data and state machine logic.

The two key components are:
   - `#[validators]` macro: Define validator methods on your persistent data struct to determine the state.
   - `to_machine` method: Call this method on your persistent data to reconstruct the state machine.

---

#### Why `#[validators]`?

The `#[validators]` macro connects **persistent data** (e.g., database rows) to your state machine in a clean, type-safe, and ergonomic way. It simplifies the process of reconstructing state machines by letting you define what the data means for each state.

##### The Key Idea:
To rebuild a state machine from persistent data, you need to define what qualifies the data as being in a specific state. For example:
- Is the data in the "Draft" state if the `status` field is `"new"`?
- Does it represent "InProgress" if additional data (e.g., `draft_version`) is present?

The `#[validators]` macro organizes this logic into validator methods—one for each state—making it easier to manage and understand.

```rust
#[validators(state = TaskState, machine = TaskMachine)]
impl DbData {
    fn is_draft(&self) -> Result<(), statum::Error> {
        if self.state == "new" {
            //Note: that we have access to the fields of TaskMachine here! 🧙
            println!("Name: {}, Priority: {}", name, priority);
            let some_other_data = fetch_data_from_somewhere(client);

            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<DraftData, statum::Error> {
        let state_data = DraftData { version: 1 };
        if self.state == "in_progress" {
            Ok(state_data)
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_complete(&self) -> Result<(), statum::Error> { /* you get the idea */ }
}
```

> Note: The fields of your machine (e.g., client, name, priority) are automatically available inside validator methods. This eliminates boilerplate by letting you directly use these fields to determine a state.

---

#### How `#[validators]` Works:

1. **Define Conditions for Each State**  
   - Each state gets a corresponding validator method (e.g., `is_draft()` for `Draft`) to determine if the persistent data matches that state. 
   - For states with extra data (e.g., `InProgress(DraftData)`), the validator method must reconstruct the necessary state-specific data.
   - A bit of macro magic allows you to directly use fields of your machine struct inside validator methods. For instance, you can use a client defined in your machine struct to fetch data needed to determine a state.

2. **Centralized Validation Logic**  
   All validation happens in one `impl` block on your persistent data struct, keeping the code organized and easy to maintain.

3. The `to_machine` Method
   The `to_machine` method is generated for your persistent data struct, which you call to reconstruct the state machine. It returns a `TaskMachineWrapper` enum that you can `match` on to handle each state.

```rust
match task_machine {
    TaskMachineWrapper::Draft(draft_machine) => { /* handle draft */ },
    TaskMachineWrapper::InProgress(in_progress_machine) => { /* handle in-progress */ },
    TaskMachineWrapper::Complete(complete_machine) => { /* handle complete */ },
}
```

---

#### Example

```rust
use serde::Serialize;
use statum::{machine, state, validators};

#[state]
#[derive(Clone, Debug, Serialize)]
pub enum TaskState {
    Draft,
    InProgress(DraftData),
    Complete,
}

#[derive(Clone, Debug, Serialize)]
pub struct DraftData {
    version: u32,
}

#[machine]
#[derive(Clone, Debug, Serialize)]
struct TaskMachine<S: TaskState> {
    client: String,
    name: String,
    priority: u8,
}

#[derive(Clone)]
struct DbData {
    id: String,
    state: String,
}

#[validators(state = TaskState, machine = TaskMachine)]
impl DbData {
    fn is_draft(&self) -> Result<(), statum::Error> {
        if self.state == "new" {
            //Note: that we have access to the fields of TaskMachine here! 🧙
            println!("Client: {}, Name: {}, Priority: {}", client, name, priority);
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<DraftData, statum::Error> {
        let state_data = DraftData { version: 1 };

        if self.state == "in_progress" {
            Ok(state_data)
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_complete(&self) -> Result<(), statum::Error> {
        if self.state == "complete" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

fn main() {
    let db_data = DbData {
        id: "123".to_owned(),
        state: "in_progress".to_owned(),
    };

    // Reconstruct the state machine
    let task_machine = db_data
        .to_machine("my_client".to_owned(), "some_name".to_owned(), 1) // Note: we pass our #[machine]'s fields here
        .unwrap();

    match task_machine {
        // Note the generated wrapper type, TaskMachineWrapper
        TaskMachineWrapper::Draft(_draft_machine) => {
            // handle_draft_machine(draft_machine);
        }
        TaskMachineWrapper::InProgress(_in_progress_machine) => {
            // handle_in_progress_machine(in_progress_machine);
        }
        TaskMachineWrapper::Complete(_complete_machine) => {
            // handle_complete_machine(complete_machine);
        }
    }
}
```
---

> **Tip:** If any of your validators are `async`, ensure you call `.to_machine()` with `.await` to avoid compilation errors.

---

## Common Errors and Tips

1. **`missing fields marker and state_data`**  
   - Usually means your derive macros (e.g., `Clone` or `Debug`) expanded before Statum could inject those fields. Move `#[machine]` above your derives, or remove them.

2. **`cannot find type X in this scope`**  
   - Ensure that you define your `#[machine]` struct _before_ you reference it in `impl` blocks or function calls.

3. **Feature gating**  
   - If you’re using `#[derive(Serialize, Deserialize)]` on a `#[state]` enum but didn’t enable the `serde` feature in Statum, you’ll get compile errors about missing trait bounds.

---

## Lint Warnings (`unexpected_cfgs`)

If you're using the nightly toolchain and you see warnings like:
```
= note: no expected values for `feature`
= help: consider adding `serde` as a feature in `Cargo.toml`
```
it means you have the `unexpected_cfgs` lint enabled but you haven’t told your crate “feature = serde” is valid. This is a Rust nightly lint that ensures you only use `#[cfg(feature="...")]` with known feature values.

To fix it, either disable the lint or declare the allowed values in your crate’s `Cargo.toml`:

```toml
[lints.rust.unexpected_cfgs]
check-cfg = [
  'cfg(feature, values("serde"))'
]
level = "warn"
```
## License

Statum is distributed under the terms of the MIT license. See [LICENSE](LICENSE) for details.
