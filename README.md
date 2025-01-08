# statum

A zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation.

## Overview

The typestate pattern lets you encode state machines at the type level, making invalid state transitions impossible at compile time. This crate makes implementing typestates effortless through two attributes:

- `#[state]` - Define your states
- `#[context]` - Create your state machine

## Installation

Add this to your `Cargo.toml`:
```toml
[dependencies]
statum = "0.1.9"
```

## Quick Start

Here's a simple example of a task processor:

```rust
use statum::{state, context};

#[state]
pub enum TaskState {
    New,
    InProgress,
    Complete,
}

#[context]
struct Task<S: TaskState> {
    id: String,
    name: String,
}

impl Task<New> {
    fn start(self) -> Task<InProgress> {
        // Use into_context() for simple state transitions
        self.into_context()
    }
}

impl Task<InProgress> {
    fn complete(self) -> Task<Complete> {
        self.into_context()
    }
}

fn main() {
    let task = Task::new(
        "task-1".to_owned(),
        "Important Task".to_owned(),
    );
    
    let task = task.start();
    let task = task.complete();
}
```

## Advanced Features

### States with Data

States can carry state-specific data:

```rust
#[state]
pub enum DocumentState {
    Draft,                      // Simple state
    Review(ReviewData),         // State with data
    Published,
}

struct ReviewData {
    reviewer: String,
    comments: Vec<String>,
}

#[context]
struct Document<S: DocumentState> {
    id: String,
    content: String,
}

impl Document<Draft> {
    fn submit_for_review(self, reviewer: String) -> Document<Review> {
        // Use into_context_with() for states with data
        self.into_context_with(ReviewData {
            reviewer,
            comments: vec![],
        })
    }
}
```

### Accessing State Data

When a state has associated data, you can access it safely:

```rust
impl Document<Review> {
    fn add_comment(&mut self, comment: String) {
        // Safely modify state data
        if let Some(review_data) = self.get_state_data_mut() {
            review_data.comments.push(comment);
        }
    }

    fn get_reviewer(&self) -> Option<&str> {
        // Safely read state data
        self.get_state_data().map(|data| data.reviewer.as_str())
    }

    fn approve(self) -> Document<Published> {
        // Transition to a state without data
        self.into_context()
    }
}
```

### Database Integration

Here's how to integrate with external data sources:

```rust
#[derive(Debug)]
enum Error {
    InvalidState,
}

#[derive(Clone)]
struct DbRecord {
    id: String,
    state: String,
}

// Convert from database record to state machine
impl TryFrom<&DbRecord> for Document<Draft> {
    type Error = Error;
    
    fn try_from(record: &DbRecord) -> Result<Self, Error> {
        if record.state != "draft" {
            return Err(Error::InvalidState);
        }
        Ok(Document::new(
            record.id.clone(),
            String::new(),
        ))
    }
}

// Or use methods for more complex conversions with data
impl DbRecord {
    fn try_to_review(&self, reviewer: String) -> Result<Document<Review>, Error> {
        if self.state != "review" {
            return Err(Error::InvalidState);
        }
        
        let doc = Document::new(
            self.id.clone(),
            String::new(),
        );
        
        Ok(doc.into_context_with(ReviewData {
            reviewer,
            comments: vec![],
        }))
    }
}
```

### Rich Context

Your state machine can maintain any context it needs:

```rust
#[context]
struct RichContext<S: DocumentState> {
    id: Uuid,
    created_at: DateTime<Utc>,
    metadata: HashMap<String, String>,
    config: Config,
}
```

## Contributing

Contributions welcome! Feel free to submit pull requests.

## License

MIT License - see LICENSE for details.
