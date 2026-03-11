use statum::{machine, state, transition};

#[state]
enum State {
    Draft(DraftDocument),
    InReview(ReviewDocument),
    Published,
}

#[machine]
struct Machine<State> {
    client: String,
}

struct DraftDocument {
    title: String,
    body: String,
}

struct ReviewDocument {
    title: String,
    body: String,
    reviewer: String,
}

#[transition]
impl Machine<Draft> {
    pub fn into_review(self, reviewer: String) -> Machine<InReview> {
        self.transition_map(|draft| ReviewDocument {
            title: draft.title,
            body: draft.body,
            reviewer,
        })
    }
}

#[transition]
impl Machine<InReview> {
    pub fn publish(self) -> Machine<Published> {
        self.transition()
    }
}

pub fn run() {
    let machine = Machine::<Draft>::builder()
        .client("docs".to_owned())
        .state_data(DraftDocument {
            title: "Spec".to_owned(),
            body: "Ship transition_map".to_owned(),
        })
        .build();

    let review = machine.into_review("Ada".to_owned());
    assert_eq!(review.client.as_str(), "docs");
    assert_eq!(review.state_data.title.as_str(), "Spec");
    assert_eq!(review.state_data.body.as_str(), "Ship transition_map");
    assert_eq!(review.state_data.reviewer.as_str(), "Ada");

    let _published = review.publish();
}
