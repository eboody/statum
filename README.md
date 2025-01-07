# statum

A zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation.

## Overview

The typestate pattern lets you encode state machines at the type level, making invalid state transitions impossible at compile time. This crate makes implementing typestates effortless through three attributes:

- `#[state]` - Define your states
- `#[context]` - Create your state machine
- `#[transition]` - Implement state transitions

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
statum = "0.1.0"
```

## Quick Start

Here's a minimal example of a task processor:

```rust
use statum::{state, context, transition};

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
}

#[transition]
impl Task<New> {
    fn start(self) -> Result<Task<InProgress>> {
        Ok(self)
    }
}

#[transition]
impl Task<InProgress> {
    async fn process(self) -> Result<Task<Complete>> {
        // Do some work...
        Ok(self)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let task = Task::new("task-1".to_string(), vec![]).await?
        .start()?
        .process()
        .await?;
    
    Ok(())
}
```

## Features

### Zero Boilerplate State Definition
```rust
#[state]
pub enum ProcessState {
    Ready,
    Working,
    Complete,
}
```

### Automatic Constructor Generation
The `#[context]` attribute automatically generates an async `new` constructor and handles the PhantomData marker:

```rust
#[context]
struct ApiClient<S: ProcessState> {
    client: reqwest::Client,
    base_url: String,
}

// Generated automatically:
impl<S: ProcessState> ApiClient<S> {
    async fn new(client: reqwest::Client, base_url: String) -> Self {
        Self {
            client,
            base_url,
            marker: PhantomData,
        }
    }
}
```

### Clean State Transitions
The `#[transition]` attribute handles the state transition machinery:

```rust
#[transition]
impl ApiClient<Ready> {
    async fn connect(self) -> Result<ApiClient<Working>> {
        // Just focus on the logic
        Ok(self)  // State transition is handled automatically
    }
}
```

### Rich Context
Your state machine can maintain any context it needs:

```rust
#[context]
struct RichContext<S: ProcessState> {
    id: Uuid,
    created_at: DateTime<Utc>,
    metadata: HashMap<String, String>,
    config: Config,
}
```

## Real World Example

Here's a more complete example showing async operations and error handling:

```rust
use enumachine::{state, context, transition};
use anyhow::Result;

#[state]
pub enum PublishState {
    Draft,
    Review,
    Published,
    Archived,
}

#[context]
struct Article<S: PublishState> {
    id: Uuid,
    content: String,
    client: ApiClient,
}

#[transition]
impl Article<Draft> {
    async fn submit_for_review(self) -> Result<Article<Review>> {
        self.client.save_draft(&self.id, &self.content).await?;
        Ok(self)
    }
}

#[transition]
impl Article<Review> {
    async fn approve(self) -> Result<Article<Published>> {
        self.client.publish(&self.id).await?;
        Ok(self)
    }
    
    async fn request_changes(self) -> Result<Article<Draft>> {
        self.client.reject(&self.id).await?;
        Ok(self)
    }
}

#[transition]
impl Article<Published> {
    async fn archive(self) -> Result<Article<Archived>> {
        self.client.archive(&self.id).await?;
        Ok(self)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let article = Article::new(
        Uuid::new_v4(),
        "My Article".to_string(),
        ApiClient::new().await,
    ).await;

    let published = article
        .submit_for_review().await?
        .approve().await?;
        
    Ok(())
}
```

## Contributing

Contributions welcome! Feel free to submit pull requests.

## License

MIT License - see LICENSE for details.
