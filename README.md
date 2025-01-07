# typestate-machine

A lightweight library for implementing the typestate pattern in Rust, enabling compile-time state machine validation.

## Overview

The typestate pattern allows you to encode state machines at the type level, making invalid state transitions impossible at compile time. This crate provides two key attributes to make implementing typestate patterns easy and ergonomic:

- `#[state]` - Defines a set of possible states for your state machine
- `#[context]` - Creates a type that can transition between states while maintaining its context

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
typestate-machine = "0.1.0"
```

## Usage

Here's a simple example of a state machine that processes a task:

```rust
use typestate_machine::{state, context};
use std::marker::PhantomData;

#[state]
pub enum TaskState {
    New,
    InProgress,
    Complete,
}

#[context]
struct Task<S: TaskState> {
    id: String,
    data: Vec<u32>,
    marker: PhantomData<S>,
}

impl Task<New> {
    fn new(id: String) -> Self {
        Task {
            id,
            data: Vec::new(),
            marker: PhantomData,
        }
    }

    fn start(self) -> Task<InProgress> {
        self.into_context()
    }
}

impl Task<InProgress> {
    fn process(mut self) -> Task<Complete> {
        // Do some work...
        self.into_context()
    }
}

impl Task<Complete> {
    fn get_results(&self) -> &[u32] {
        &self.data
    }
}

fn main() {
    let task = Task::new("task-1".to_string())
        .start()
        .process();
    
    let results = task.get_results();
}
```

## How It Works

### The `#[state]` Attribute

The `#[state]` attribute transforms an enum into a set of unit structs that implement a common trait. For example:

```rust
#[state]
pub enum State {
    Ready,
    Working,
    Complete,
}
```

Gets transformed into:

```rust
pub trait State {}
pub struct Ready;
pub struct Working;
pub struct Complete;
impl State for Ready {}
impl State for Working {}
impl State for Complete {}
```

### The `#[context]` Attribute

The `#[context]` attribute implements the machinery needed for safe, ergonomic state transitions. It:

1. Creates an `IntoContext` trait
2. Implements the trait for your type
3. Maintains all context fields during state transitions

This allows you to write state transitions using the ergonomic `.into_context()` method.

## Best Practices

1. Always name your states descriptively - they represent the specific states your entity can be in
2. Keep your context struct focused - it should only contain data that needs to persist across state transitions
3. Implement state-specific methods only on the relevant state variants
4. Use descriptive method names for state transitions (e.g., `start()`, `complete()`, `process()`)

## Advanced Usage

### Async State Machines

The library works seamlessly with async code:

```rust
#[context]
struct AsyncTask<S: TaskState> {
    client: reqwest::Client,
    url: String,
    marker: PhantomData<S>,
}

impl AsyncTask<New> {
    async fn fetch(self) -> Result<AsyncTask<InProgress>> {
        // Do async work...
        Ok(self.into_context())
    }
}
```

### Error Handling

You can wrap state transitions in `Result` to handle potential failures:

```rust
impl Task<InProgress> {
    fn complete(self) -> Result<Task<Complete>, Error> {
        // Check conditions...
        if something_wrong {
            return Err(Error::FailedToComplete);
        }
        Ok(self.into_context())
    }
}
```

## Contributing

Contributions are welcome! Please feel free to submit pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
