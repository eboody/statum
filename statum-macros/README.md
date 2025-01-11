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

// 2. Create a machine struct that references one of those states.
#[machine]
pub struct Light<S: LightState> {
    name: String,
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

States can hold data. For example:

```rust
#[state]
pub enum ReviewState {
    Draft,
    InReview(ReviewData),
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
        self.transition_with(data)
    }
}

// ...
```

#### Accessing State Data

Use `.get_state_data()` or `.get_state_data_mut()` to interact with the state-specific data:

```rust
impl Document<Review> {
    fn add_comment(&mut self, comment: String) {
        if let Some(review_data) = self.get_state_data_mut() {
            review_data.comments.push(comment);
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

### 4. Attribute Ordering

- **`#[state]`** must go on an **enum**.  
- **`#[machine]`** must go on a **struct**.  
- Because `#[machine]` injects extra fields, you need it _above_ any user `#[derive(...)]`. If you place `#[derive(...) ]` first, you might see “missing fields `marker` and `state_data` in initializer” errors.

---
### 5. Database Integration

statum provides a seamless way to convert database entries into state machines using a declarative macro-based approach. By combining #[model], #[state], and #[machine] attributes, it simplifies state validation and transition logic for Rust applications.

- Declarative syntax for linking database models to state machines.
- Automated generation of try_to_* methods for state validation and transition.
- Flexible validation logic using user-defined is_* methods.
- Type-safe and ergonomic state machine transitions.
- Clear error handling via StatumError.

#### Defining Your State Machine
Define your state machine states using the #[state] and #[machine] macros:
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
```

#### Linking Database Models with #[model]
Use the #[model] attribute to link your database representation to the state machine:

```rust
#[model(state = TaskState, machine = TaskMachine)]
#[derive(Clone)]
struct DbData {
    id: String,
    state: String,
}
```

#### Writing Validators
Define custom is_* methods for each state variant to validate transitions:

```rust
impl DbData {
    fn is_new(&self) -> bool {
        self.state == "new"
    }

    fn is_in_progress(&self) -> bool {
        self.state == "in_progress"
    }

    fn is_complete(&self) -> bool {
        self.state == "complete"
    }
}
```
These methods provide fine-grained control over the conditions that must be met for each state.

#### Generated try_to_* Methods

The #[model] macro automatically generates try_to_* methods for each state variant:

```rust
impl DbData {
    pub fn try_to_new(&self, client: String) -> Result<TaskMachine<New>, StatumError> {
        if self.is_new() {
            Ok(TaskMachine::new(client))
        } else {
            Err(StatumError::InvalidState)
        }
    }

    pub fn try_to_in_progress(&self, client: String) -> Result<TaskMachine<InProgress>, StatumError> {
        if self.is_in_progress() {
            Ok(TaskMachine::new(client))
        } else {
            Err(StatumError::InvalidState)
        }
    }

    pub fn try_to_complete(&self, client: String) -> Result<TaskMachine<Complete>, StatumError> {
        if self.is_complete() {
            Ok(TaskMachine::new(client))
        } else {
            Err(StatumError::InvalidState)
        }
    }
}
```
These methods are:
- Type-safe: Ensure only valid transitions are allowed.
- Easy to use: Automatically wired to the custom validators we made earlier.

#### Example workflow
```rust
fn main() {
    let client = "mock_client".to_owned();
    let task = DbData {
        id: "42".to_owned(),
        state: "new".to_owned(),
    };

    if let Ok(machine) = task.try_to_new(client.clone()) {
        let machine = machine.start();
        let machine = machine.process();
    } else if let Ok(machine) = task.clone().try_to_in_progress(client.clone()) {
        let machine = machine.process();
    }
}
```
- Start with a DbData object representing a database entry.
- Use try_to_* methods to validate and transition to the appropriate state.
- Call state-specific methods (start, process, etc.) on the machine.

#### Error Handling
All try_to_* methods return a Result with:
- Ok: A valid TaskMachine instance in the desired state.
- Err(StatumError): An error indicating invalid state transitions.

### 6. Implementing the Typestate Builder Pattern with Statum

The **typestate builder pattern** is a powerful way to enforce correct usage of a sequence of steps at compile time. With **statum**, you can implement this pattern using the provided `#[state]` and `#[machine]` macros to ensure type-safe state transitions in your builders.

This guide will walk you through implementing a typestate builder for a hypothetical "User Registration" workflow.

---

#### Overview

Imagine we have a multi-step process for registering a user:
1. Collect the user's name.
2. Set the user's email.
3. Submit the registration.

Using the typestate builder pattern, we can ensure:
- Each step must be completed before moving to the next.
- Skipping steps or submitting prematurely results in compile-time errors.

---

#### Steps to Implement

##### 1. Define States

Each step in the builder process is represented as a state using `#[state]`. For example:

```rust
use statum::{state, machine};

#[state]
pub enum UserState {
    NameNotSet,
    NameSet(NameData),
    EmailSet(UserData),
}

#[derive(Debug, Clone)]
pub struct NameData {
    name: String,
}

#[derive(Debug, Clone)]
pub struct UserData {
    name: String,
    email: String,
}
```

Here:
- **`NameNotSet`**: The initial state where the name has not been set.
- **`NameSet`**: The state where the name is provided but the email is not.
- **`EmailSet`**: The final state before submission.

##### 2. Create the Builder Machine

The builder itself is a `#[machine]`-decorated struct that uses the defined states.

```rust
#[machine]
#[derive(Debug, Clone)]
pub struct UserBuilder<S: UserState> {
    id: u32,
}
```

This struct will manage transitions between states.

##### 3. Define State Transitions

Implement methods to move from one state to the next:

###### Transition from `NameNotSet` to `NameSet`
```rust
impl UserBuilder<NameNotSet> {
    pub fn set_name(self, name: String) -> UserBuilder<NameSet> {
        let data = NameData { name };
        self.transition_with(data)
    }
}
```

###### Transition from `NameSet` to `EmailSet`
```rust
impl UserBuilder<NameSet> {
    pub fn set_email(self, email: String) -> UserBuilder<EmailSet> {
        let NameData { name } = self.get_state_data().unwrap().clone();
        let data = UserData { name, email };
        self.transition_with(data)
    }
}
```

###### Transition from `EmailSet` to Submission
```rust
impl UserBuilder<EmailSet> {
    pub fn submit(self) -> Result<(), &'static str> {
        let UserData { name, email } = self.get_state_data().unwrap();
        println!("User registered: Name = {}, Email = {}", name, email);
        Ok(())
    }
}
```

##### 4. Example Usage

Here’s how you would use this builder:

```rust
fn main() {
    let builder = UserBuilder::<NameNotSet>::new(1);

    // Step 1: Set the name
    let builder = builder.set_name("Alice".to_string());

    // Step 2: Set the email
    let builder = builder.set_email("alice@example.com".to_string());

    // Step 3: Submit the registration
    builder.submit().unwrap();
}
```

##### Compile-Time Guarantees

- You **cannot** set the email without first setting the name:
  ```rust
  let builder = UserBuilder::<NameNotSet>::new(1);
  let builder = builder.set_email("alice@example.com".to_string()); // Compile-time error
  ```

- You **cannot** submit the builder without setting the name and email:
  ```rust
  let builder = UserBuilder::<NameNotSet>::new(1);
  builder.submit(); // Compile-time error
  ```

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
