# Statum

**Statum** is a zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation. It provides two attribute macros:

- **`#[state]`** for defining states (as enums).
- **`#[machine]`** for creating a state machine struct that tracks which state you’re in at compile time.

## Quick Start (Minimal Example)

Here’s the simplest usage of Statum without any extra features:

```rust
use statum::{state, machine};

// 1. Define your states as an enum.
#[state]
pub enum LightState {
    Off,
    On,
}

#[machine]
pub struct Light<S: LightState> {
    name: String, // Contextual, Machine-wide fields go here, like clients, configs, an identifier, etc.
}

// 3. Implement transitions for each state.
impl Light<Off> {
    pub fn switch_on(self) -> Light<On> {
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
    // note: you dont need the ::<Off> here, it is inferred
    // but it is shown here for clarity
    let light = Light::<Off>::new("desk lamp".to_owned());

    // 5. Transition from Off -> On, On -> Off, etc.
    let light = light.switch_on();
    let light = light.switch_off();
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

That’s because the derive macro for `Clone`, `Debug`, etc., expands before `#[machine]` has injected these extra fields. **To avoid this**, either:

- Put `#[machine]` _above_ the derive(s), or  
- Remove the conflicting derive(s) from the same item.

For example, this works:

```rust
#[machine]
#[derive(Debug, Clone)]
pub struct Light<S: LightState> {
    name: String,
}
```
This does not:
```rust
#[derive(Debug, Clone)] //note the position of the derive
#[machine]
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
use statum::state;

#[state]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum DocumentState {
    Draft,
    Published,
}
```

If you enable Statum’s `"serde"` feature, any `#[derive(Serialize)]` and `#[derive(Deserialize)]` you put on the enum will get passed through to the expanded variant structs. If you do **not** enable that feature, deriving those traits will likely fail to compile.

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

#[derive(Debug)]
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

#[derive(Clone)]
struct DbData {
    id: String,
    state: String,
}

#[validators(state = TaskState, machine = TaskMachine)]
impl DbData {
    fn is_new(&self) -> Result<(), statum::Error> {
        if self.state == "new" {
            //Note: that we have access to the fields of TaskMachine here!
            println!("Client: {}, Name: {}, Priority: {}", client, name, priority);
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<DraftData, statum::Error> {
        if self.state == "in_progress" {
            println!("Client: {}, Name: {}, Priority: {}", client, name, priority);
            Ok(DraftData { version: 1 })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_complete(&self) -> Result<(), statum::Error> {
        if state == "complete" {
            println!("Client: {}, Name: {}, Priority: {}", client, name, priority);
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
    let task_machine = db_data.to_machine().unwrap();

    // Match on the state machine wrapper to access state-specific logic
    match task_machine {
        TaskMachineWrapper::New => println!("Task is new"),
        TaskMachineWrapper::InProgress(data) => println!("Task is in progress with version: {}", data.version),
        TaskMachineWrapper::Complete => println!("Task is complete"),
    }
}
```

In this example, the `#[validators]` macro ensures that:
1. Fields of the machine (`client`, `name`, `priority`) are **automatically available** inside validator methods.
2. `db_data.to_machine()` calls the macro-generated `to_machine` method to determine the appropriate state and reconstruct the state machine.
3. Using `match` on `TaskMachineWrapper`, the reconstructed machine's state determines the behavior, ensuring type-safe and intuitive handling

---

#### Why Validators?

The `#[validators]` macro exists to solve a key problem: **connecting persistent data to state machines** in a type-safe, ergonomic, and flexible way. Here’s why it’s essential:

1. **Bridging Persistent Data and States**  
   Persistent data, like database records, must be interpreted to determine the correct state of a machine. Validators provide a clear and structured way to define how a given record maps to each state.

2. **Simplifying Validation Logic**  
   Validators allow you to define **state-specific conditions** and encapsulate them in dedicated methods. This makes complex logic easier to manage and reduces boilerplate code.

3. **Direct Access to Machine Fields**  
   Instead of manually passing fields to validation methods, the `#[validators]` macro automatically makes them available. This simplifies the code and keeps validation logic focused on its purpose.

4. **Ensuring Completeness and Safety**  
   By centralizing validation logic in a single `#[validators]`-annotated block:
   - Each state is guaranteed to have a corresponding validator.
   - Rust's type system ensures the logic is correct and consistent.

5. **Constructing State-Specific Data**  
   Validators can generate data specific to certain states (e.g., `DraftData` for `InProgress`). This encapsulation ensures the state machine is reconstructed with integrity and minimal duplication.

---

#### Macro-Generated Reconstruction

The `#[validators]` macro also generates a `to_machine` method that automates the process of:
1. Validating the state using the corresponding methods.
2. Constructing the state machine with the correct state and any state-specific data.

---

**Tip:** If your validators are `async`, ensure you call `.to_machine()` with `.await` to avoid compilation errors.

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

If you see warnings like:
```
= note: no expected values for `feature`
= help: consider adding `foo` as a feature in `Cargo.toml`
```
it means you have the `unexpected_cfgs` lint enabled but you haven’t told your crate “feature = foo” is valid. This is a Rust nightly lint that ensures you only use `#[cfg(feature="...")]` with known feature values.

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
