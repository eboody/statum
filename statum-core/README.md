<div align="center">
    <img src="https://github.com/eboody/statum/raw/main/logo.png" alt="statum Logo" width="150">
</div>

# Statum

**Statum** is a zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation. To start, it provides two attribute macros:

- **`#[state]`** for defining states (as enums).
- **`#[machine]`** for creating a state machine struct that tracks which state you’re in at compile time.

There is one more super useful macro, but read on to find out more!

## Quick Start (Minimal Example)

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

That’s because the derive macro for `Clone`, `Debug`, etc., expands before `#[machine]` has injected these extra fields. 
**To avoid this**, put `#[machine]` _above_ the derive(s).

```rust
// ❌ This will cause an error
#[derive(Debug, Clone)] // ↩ note the position of the derive
#[machine]
pub struct Light<S: LightState> {
    name: String,
}

// ✅ This will work
#[machine]
#[derive(Debug, Clone)]
pub struct Light<S: LightState> {
    name: String,
}

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
        self.transition_with(data) // Note: when we have state data, we use self.transition_with(...) instead of self.transition()
    }
}

// ...
```

We use `self.transition_with(data)` instead of `self.transition()` to transition to a state that carries data.

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

---

#### Using `#[validators]` to Reconstruct State Machines

Here's a quick example to illustrate how `#[validators]` helps reconstruct state machines from persistent data:

```rust
use serde::Serialize;
use statum::{machine, state, validators};

#[state]
#[derive(Clone, Debug, Serialize)]
pub enum TaskState {
    New,
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

#[derive(Clone)] // the struct that represents our persistent data
struct DbData {
    id: String,
    state: String,
}


// Define validators for each state
// Note: the validator method names are the same as the state variants but begin with is_*
#[validators(state = TaskState, machine = TaskMachine)]
impl DbData {
    fn is_new(&self) -> Result<(), statum::Error> {
        // a contrived validation check
        if self.state == "new" {
            //Note: that we have access to the fields of TaskMachine here! 🧙
            println!("Client: {}, Name: {}, Priority: {}", client, name, priority);
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<DraftData, statum::Error> {
        // We must return state-specific data defined in the state enum
        // It is in these validators that we reconstruct the state data from
        // our persistent data
        let state_data = DraftData { version: 1 };

        if self.state == "in_progress" {
            Ok(state_data)
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    // statum plays nicely with tokio
    async fn is_complete(&self) -> Result<(), statum::Error> {
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
    // Note: we use the to_machine method generated by the #[validators] macro
    let task_machine = db_data
        .to_machine("my_client".to_owned(), "some_name".to_owned(), 1) // Note: we pass the #[machine]'s fields here
        .unwrap();

    // Match on the state machine wrapper to access state-specific logic
    match task_machine {
        // Note the generated wrapper type, TaskMachineWrapper
        TaskMachineWrapper::New(_new_machine) => {
            // handle_new_machine(new_machine);
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

In this example, the `#[validators]` macro ensures that:
1. Fields of the machine (`client`, `name`, `priority`) are **automatically available** inside validator methods.
2. `db_data.to_machine()` calls the macro-generated `to_machine` method to determine the appropriate state and reconstruct the state machine.
3. Using `match` on `TaskMachineWrapper`, the reconstructed machine's state determines the behavior, ensuring type-safe and intuitive handling

---

#### Why `#[validators]`?

The `#[validators]` macro exists to solve a key problem: **connecting persistent data to state machines** in a type-safe, ergonomic, and flexible way.

1. **Defining State Conditions for Persistent Data:**
   
   When data is stored persistently (e.g., in a database), it typically includes information about the current state of an entity. To accurately reconstruct the state machine from this data, we must clearly define **what it means** for the data to be in each possible state of the machine.

2. **Handling Complex Validation Logic:**
   
   Determining the state based on persistent data can be intricate. Various fields, relationships, or external factors might influence the state determination. Statum provides the flexibility for developers to implement **custom validation logic** tailored to their specific requirements.

3. **Organized Validation via impl Blocks:**
   
   By defining validation methods within an impl block on the persistent data struct (e.g., DbData), `statum` ensures that there is a **dedicated method for each state variant**. This organization:
   
   - **Enforces Completeness:** Guarantees that every state has an associated validator.
   - **Enhances Readability:** Centralizes state-related validation logic, making the codebase easier to understand and maintain.
   - **Leverages Rust’s Type System:** Ensures that validations are type-safe and integrated seamlessly with the rest of the Rust code.

4. **Constructing State-Specific Data Within Validators:**
   
   For states that carry additional data (e.g., InProgress(DraftData)), the validator methods are responsible for **constructing the necessary state-specific data**. This design choice ensures that:
   
   - **Data Integrity:** The state machine is instantiated with all required data, maintaining consistency and preventing runtime errors.
   - **Encapsulation:** The logic for creating state-specific data is encapsulated within the validator, keeping the reconstruction process clean and modular.
   - **Flexibility:** Developers can define exactly how state-specific data is derived from persistent data, accommodating diverse and complex scenarios.

---

#### Macro-Generated Reconstruction

The `#[validators]` macro also generates a `to_machine` method that automates the process of:
1. Validating the state using the corresponding methods.
   - It does this by generated try_from implementations for each state.
2. Constructing the state machine with the correct state and any state-specific data.

---

**Tip:** If any of your validators are `async`, ensure you call `.to_machine()` with `.await` to avoid compilation errors.

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
