# A Stronger Minimal Example

The `New -> InProgress -> Complete` shape is fine for syntax, but it is a weak
value example because a plain wrapper or ordinary builder can cover most of it.

This is a better minimal example because it shows:

- methods that should only exist in certain states
- data that only exists in certain states
- transitions that carry state-specific payloads

Review and publish data only appear once the article reaches those states.

```rust
use statum::{machine, state, transition};

#[state]
pub enum ArticleState {
    Draft,
    InReview(ReviewAssignment),
    Published(PublishedReceipt),
}

pub struct ReviewAssignment {
    reviewer: String,
}

pub struct PublishedReceipt {
    published_at: String,
}

#[machine]
pub struct Article<ArticleState> {
    id: String,
    title: String,
    body: String,
}

impl Article<Draft> {
    pub fn edit_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }
}

#[transition]
impl Article<Draft> {
    pub fn submit(self, reviewer: String) -> Article<InReview> {
        self.transition_with(ReviewAssignment { reviewer })
    }
}

impl Article<InReview> {
    pub fn reviewer(&self) -> &str {
        &self.state_data.reviewer
    }
}

#[transition]
impl Article<InReview> {
    pub fn approve(self, published_at: String) -> Article<Published> {
        self.transition_with(PublishedReceipt { published_at })
    }
}

impl Article<Published> {
    pub fn public_url(&self) -> String {
        format!("/articles/{}", self.id)
    }
}

fn main() {
    let draft = Article::<Draft>::builder()
        .id("post-1".to_owned())
        .title("Why Typestate Helps".to_owned())
        .body("Draft body".to_owned())
        .build()
        .edit_body("Final body".to_owned());
    // draft is Article<Draft>

    let review = draft.submit("alice".to_owned());
    // review is Article<InReview>

    let article = review.approve("2026-03-17T12:00:00Z".to_owned());
    // article is Article<Published>

    assert_eq!(article.public_url(), "/articles/post-1");
}
```

Why this is a better Statum example:

- `edit_body(...)` only exists on `Article<Draft>`
- `reviewer()` only exists on `Article<InReview>`
- `public_url()` only exists on `Article<Published>`
- review metadata does not exist until the machine is actually in review
- publish metadata does not exist until the machine is actually published

Statum models a workflow as legal states with a legal API in each phase. The
compiler enforces that shape.
