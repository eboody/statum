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

In real-world applications, state machines often need to **persist their state**—for instance, saving to and loading from a database. Reconstructing a state machine from such persistent data requires a robust and type-safe mechanism to ensure that the machine accurately reflects the stored state. With Statum, this process is seamless, intuitive, and developer-friendly.

#### Key Feature: Direct Access to Machine Fields Inside Validator Methods

Statum's `#[validators]` macro enables you to directly access **all fields of your machine struct** inside validator methods. This means you don't need to manually define or pass these fields as arguments—they are automatically injected and accessible. This enhancement drastically reduces boilerplate and ensures your validators focus solely on their logic.

For example, if your machine struct has fields like `client`, `name`, or any other properties, these will be accessible inside your validator methods without any extra effort.

---

#### Motivation

The `#[validators]` macro is designed to bridge the gap between **persistent data** (e.g., database records) and **state machines** by providing an ergonomic, type-safe, and flexible way to reconstruct state machines. Here’s the reasoning behind this design:

1. **Defining State Conditions for Persistent Data:**
   - Persistent data typically includes information about the current state of an entity, often stored as a string or similar identifier.
   - To reconstruct a state machine accurately, we need to define **what it means** for the data to represent each possible state of the machine.
   - `validators` provides a structured mechanism to associate state conditions with their corresponding state variants, ensuring clarity and correctness.

2. **Handling Complex Validation Logic:**
   - State determination is rarely straightforward; it might depend on a combination of fields, relationships, or external factors.
   - With `validators`, developers can implement **custom validation logic** tailored to their application's requirements while benefiting from Rust's safety and expressiveness.

3. **Simplifying Access to Machine Fields:**
   - Validators often need access to fields of the state machine (e.g., configuration, context) to perform their checks or construct state-specific data.
   - To eliminate boilerplate, the `#[validators]` macro automatically injects all fields of the machine into validator methods, making them directly accessible.
   - This ensures developers can focus on validation logic without worrying about passing or defining these fields manually.

4. **Organized and Complete State Validation:**
   - By defining validators within an `impl` block annotated with `#[validators]`, Statum ensures that:
     - Every state variant has a dedicated validator.
     - The validation logic is centralized and easy to maintain.
     - Rust's type system guarantees that validators are complete, consistent, and type-safe.

5. **Constructing State-Specific Data Within Validators:**
   - Some states (e.g., `InProgress(DraftData)`) carry additional data that needs to be reconstructed along with the state machine.
   - Validator methods are responsible for constructing this state-specific data, ensuring:
     - **Data Integrity:** The machine is reconstructed with all required data.
     - **Encapsulation:** State-specific logic is localized within the validators.
     - **Flexibility:** Developers can define how state-specific data is derived, supporting complex scenarios.

6. **Macro-Generated Reconstruction for Consistency and Simplicity:**
   - The macro-generated `to_machine` method automates the reconstruction of the state machine by:
     - Calling each validator to determine the state and retrieve any associated data.
     - Constructing the state machine with the appropriate fields and state-specific data.
   - This ensures consistency across the codebase and reduces the risk of errors.

---

#### How It Works

1. **Define States and Machine:**
   - Use the `#[state]` macro to define your state enum, specifying which states carry additional data.
   - Use the `#[machine]` macro to create the state machine struct and register its fields.

2. **Define Persistent Data and Implement Validators:**
   - Define a struct that represents your persistent data (e.g., a database record).
   - Annotate an `impl` block on this persistent data struct with `#[validators(state = YourState, machine = YourMachine)]`.
   - Implement a validator method for each state variant, named as `is_*`, where `*` corresponds to the snake_case version of the state name.
   - Inside each validator:
     - Directly access the machine's fields as if they were local variables.
     - Use these fields to determine if the persistent data matches the state and construct any state-specific data.

3. **Macro-Generated Reconstruction:**
   - The `#[validators]` macro automatically:
     - Augments your `impl` block to make the machine’s fields available in validator methods.
     - Generates a `to_machine` method to:
       - Validate each state using the corresponding validator method.
       - Construct the state machine with the appropriate fields and state-specific data.
     - Returns a type-safe wrapper enum that encapsulates the reconstructed state machine.

---

#### Example

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
            println!("Client: {}, Name: {}, Priority: {}", self.client, self.name, self.priority);
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<DraftData, statum::Error> {
        if self.state == "in_progress" {
            println!("Client: {}, Name: {}, Priority: {}", self.client, self.name, self.priority);
            Ok(DraftData { version: 1 })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_complete(&self) -> Result<(), statum::Error> {
        if self.state == "complete" {
            println!("Client: {}, Name: {}, Priority: {}", self.client, self.name, self.priority);
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}
```

In this example:
- **Automatic field injection:** The fields `client`, `name`, and `priority` from `TaskMachine` are directly available inside the validator methods, reducing boilerplate and improving readability.
- **Generic handling:** This behavior applies to **any fields defined in the machine struct**, no matter their names or types.

---

Statum provides an ergonomic and powerful way to integrate persistent data with state machines, giving developers the flexibility and safety they need to build robust systems.

---

#### Why This Matters

1. **Simplifies Validators:**
   - You no longer need to manually define arguments for every field in your state machine.
   - Validators focus solely on their logic.

2. **Improves Developer Productivity:**
   - Reduces repetitive code, making the implementation faster and cleaner.

3. **Ensures Consistency:**
   - The same fields are used consistently across all states and validators, minimizing errors.

---

By integrating field access into validator methods, Statum makes it even easier to build robust, type-safe state machines that integrate seamlessly with persistent data. 

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
