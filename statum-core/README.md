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

### 4. Reconstructing State Machines from Persistent Data

In real-world applications, state machines often need to **persist their state**—for instance, saving to and loading from a database. Reconstructing a state machine from such persistent data requires a robust and type-safe mechanism to ensure that the machine accurately reflects the stored state. Here's how Statum facilitates this process:

#### Motivation

1. **Defining State Conditions for Persistent Data:**
   
   When data is stored persistently (e.g., in a database), it typically includes information about the current state of an entity. To accurately reconstruct the state machine from this data, we must clearly define **what it means** for the data to be in each possible state of the machine.

2. **Handling Complex Validation Logic:**
   
   Determining the state based on persistent data can be intricate. Various fields, relationships, or external factors might influence the state determination. Statum provides the flexibility for developers to implement **custom validation logic** tailored to their specific requirements.

3. **Organized Validation via `impl` Blocks:**
   
   By defining validation methods within an `impl` block on the persistent data struct (e.g., `DbData`), Statum ensures that there is a **dedicated method for each state variant**. This organization:
   
   - **Enforces Completeness:** Guarantees that every state has an associated validator.
   - **Enhances Readability:** Centralizes state-related validation logic, making the codebase easier to understand and maintain.
   - **Leverages Rust’s Type System:** Ensures that validations are type-safe and integrated seamlessly with the rest of the Rust code.

4. **Constructing State-Specific Data Within Validators:**
   
   For states that carry additional data (e.g., `InProgress(DraftData)`), the validator methods are responsible for **constructing the necessary state-specific data**. This design choice ensures that:
   
   - **Data Integrity:** The state machine is instantiated with all required data, maintaining consistency and preventing runtime errors.
   - **Encapsulation:** The logic for creating state-specific data is encapsulated within the validator, keeping the reconstruction process clean and modular.
   - **Flexibility:** Developers can define exactly how state-specific data is derived from persistent data, accommodating diverse and complex scenarios.

#### How It Works

1. **Define States and Machine:**
   
   - Use the `#[state]` macro to define your state enum, specifying which states carry additional data.
   - Use the `#[machine]` macro to create the state machine struct, registering any fields that are required across states.

2. **Define Persistent Data and Implement Validators:**
   
   - Define a struct that represents your persistent data (e.g., a database record).
   - Annotate an `impl` block on this persistent data struct with `#[validators(state = YourState, machine = YourMachine)]`.
   - Within this block, implement a validator method for each state variant. **Each method must be named following the pattern `is_*`, where `*` is the snake_case version of the corresponding state variant.** For example, for a state `InProgress`, implement a method named `fn is_in_progress(&self) -> Result<…, …>`.
   - These methods should:
     - **Check State Validity:** Determine if the persistent data corresponds to the specific state.
     - **Construct State Data (if needed):** For data-bearing states, create and return the necessary associated data.
        - In the scneario where you have state-specific data, your validator must return Result<YourData, statum::Error> instead of Result<(), statum::Error>.

3. **Macro-Generated Reconstruction:**
   
   - The `#[validators]` macro analyzes the validator methods and the state machine’s field information.
   - It generates a `to_machine` method on your persistent data struct that:
     - **Invokes Validators:** Calls each validator to check the state and retrieve any associated data.
     - **Constructs the State Machine:** Instantiates the state machine in the correct state, passing in the required fields and data.
     - **Ensures Type Safety:** Returns a wrapper enum that encapsulates the correctly typed state machine, preventing invalid state transitions at compile time.

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
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    //Note: this method returns the state-specific data because the state is InProgress
    //which carries additional data
    fn is_in_progress(&self) -> Result<DraftData, statum::Error> {
        if self.state == "in_progress" {
            Ok(DraftData { version: 1 })
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

impl TaskMachine<New> {
    fn start(self) -> TaskMachine<InProgress> {
        let draft_data = DraftData { version: 1 };
        self.transition_with(draft_data)
    }
}

impl TaskMachine<InProgress> {
    fn process(self) -> TaskMachine<Complete> {
        self.transition()
    }
}

fn main() {
    let client = "mock_client".to_owned();

    let task = DbData {
        id: "42".to_owned(),
        state: "in_progress".to_owned(),
    };

    let machine = task.to_machine(client).unwrap();

    match machine {
        TaskMachineWrapper::New(machine) => {
            println!("Task is new");
            let machine = machine.start();
            let machine = machine.process();
        }
        TaskMachineWrapper::InProgress(machine) => {
            println!("Task is in progress");
            let data = machine.get_state_data().unwrap();
            println!("data: {:#?}", data);
            let machine = machine.process();
            println!("machine: {:#?}", machine);
        }
        TaskMachineWrapper::Complete(machine) => {
            println!("Task is complete");
        }
    }
}
```

By integrating validation methods within `impl` blocks and leveraging macros to enforce and utilize these validations, Statum provides a powerful and ergonomic way to bridge persistent data with compile-time validated state machines.

Note: your validators can be async but make sure your call to .to_machine() is also async. You'll get a nice error message if you forget to do this.

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
