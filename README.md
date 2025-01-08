# statum

A zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation.

## Overview

The typestate pattern lets you encode state machines at the type level, making invalid state transitions impossible at compile time. This crate makes implementing typestates effortless through two attributes:

- `#[state]` - Define your states and their associated data
- `#[context]` - Create your state machine

## Installation

Add this to your `Cargo.toml`:
```toml
[dependencies]
statum = "0.1.8"
```

## Quick Start

Here's a simple example of a document workflow:

```rust
use statum::{state, context};

#[state]
pub enum DocumentState {
    Draft,                      // A simple state with no data
    Review(ReviewData),         // A state with associated data
    Published,
}

// Data associated with the Review state
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
        let review_data = ReviewData {
            reviewer,
            comments: vec![],
        };
        // Use into_context_with() when transitioning to a state with data
        self.into_context_with(review_data)
    }
}

impl Document<Review> {
    fn approve(self) -> Document<Published> {
        // Use into_context() when transitioning to a state without data
        self.into_context()
    }
}

fn main() {
    let doc = Document::new("doc-1".to_owned(), "Hello".to_owned())
        .submit_for_review("Alice".to_owned())
        .approve();
}
```

## Features

### Flexible State Definitions

States can be simple markers or carry data specific to that state:

```rust
#[state]
pub enum ProcessState {
    // Simple states without data
    Ready,
    Complete,
    
    // States with associated data
    Working(WorkProgress),
    Failed(ErrorInfo),
}

struct WorkProgress {
    started_at: DateTime<Utc>,
    percent_complete: f32,
}

struct ErrorInfo {
    error: String,
    retry_count: u32,
}
```

### Type-Safe State Transitions

The library provides two methods for state transitions:

- `into_context()` - For transitioning to states without data
- `into_context_with(data)` - For transitioning to states with associated data

This ensures you can't forget to provide required state data:

```rust
impl Process<Ready> {
    fn start(self) -> Process<Working> {
        let progress = WorkProgress {
            started_at: Utc::now(),
            percent_complete: 0.0,
        };
        // Must use into_context_with() because Working carries data
        self.into_context_with(progress)
    }
}

impl Process<Working> {
    fn complete(self) -> Process<Complete> {
        // Can use into_context() because Complete has no data
        self.into_context()
    }
    
    fn fail(self, error: String) -> Process<Failed> {
        let error_info = ErrorInfo {
            error,
            retry_count: 0,
        };
        self.into_context_with(error_info)
    }
}
```

### Automatic Constructor Generation

The `#[context]` attribute automatically generates a `new` constructor:

```rust
#[context]
struct ApiClient<S: ProcessState> {
    client: reqwest::Client,
    base_url: String,
}

// Generated automatically:
impl<S: ProcessState> ApiClient<S> {
    fn new(client: reqwest::Client, base_url: String) -> Self {
        Self {
            client,
            base_url,
            marker: PhantomData,
        }
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

Here's a more complete example showing async operations and state transitions with data:

```rust
use statum::{state, context};
use anyhow::Result;

#[state]
pub enum PublishState {
    Draft,
    Review(ReviewMetadata),
    Published(PublishInfo),
    Archived,
}

struct ReviewMetadata {
    reviewer: String,
    deadline: DateTime<Utc>,
}

struct PublishInfo {
    published_at: DateTime<Utc>,
    published_by: String,
}

#[context]
struct Article<S: PublishState> {
    id: Uuid,
    content: String,
    client: ApiClient,
}

impl Article<Draft> {
    async fn submit_for_review(self, reviewer: String) -> Result<Article<Review>> {
        self.client.save_draft(&self.id, &self.content).await?;
        
        let metadata = ReviewMetadata {
            reviewer,
            deadline: Utc::now() + Duration::days(7),
        };
        Ok(self.into_context_with(metadata))
    }
}

impl Article<Review> {
    async fn approve(self, approver: String) -> Result<Article<Published>> {
        self.client.publish(&self.id).await?;
        
        let publish_info = PublishInfo {
            published_at: Utc::now(),
            published_by: approver,
        };
        Ok(self.into_context_with(publish_info))
    }
    
    async fn request_changes(self) -> Result<Article<Draft>> {
        self.client.reject(&self.id).await?;
        Ok(self.into_context())
    }
}

impl Article<Published> {
    async fn archive(self) -> Result<Article<Archived>> {
        self.client.archive(&self.id).await?;
        Ok(self.into_context())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let article = Article::new(
        Uuid::new_v4(),
        "My Article".to_string(),
        ApiClient::new().await,
    );
    
    let published = article
        .submit_for_review("reviewer@example.com".to_owned()).await?
        .approve("editor@example.com".to_owned()).await?;
        
    Ok(())
}
```

## Contributing

Contributions welcome! Feel free to submit pull requests.

## License

MIT License - see LICENSE for details.
