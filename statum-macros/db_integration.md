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
