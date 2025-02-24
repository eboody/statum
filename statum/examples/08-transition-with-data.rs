use statum::{machine, state, transition};

#[state]
enum State {
    Draft(MyDraft),
    InReview(DraftWithComment),
    Published,
}

#[machine]
struct Machine<State> {}

struct MyDraft {
    _title: String,
    _content: String,
}

struct DraftWithComment {
    _draft: MyDraft,
    _comment: String,
}

#[transition]
impl Machine<Draft> {
    pub fn _into_review(self) -> Machine<InReview> {
        let my_draft: &MyDraft = &self.state_data;

        let draft_with_comment = DraftWithComment {
            _draft: MyDraft {
                _title: my_draft._title.clone(),
                _content: my_draft._content.clone(),
            },
            _comment: "This is a comment".to_owned(),
        };

        // NOTE: when transitioning to InReview, we need to provide the transition method with the
        // next state's data. In this case, we are transitioning to InReview and we need to provide DraftWithComment
        // This will make the data available for use in the next state
        self.transition_with(draft_with_comment)
    }
}

#[transition]
impl Machine<InReview> {
    pub fn _into_published(self) -> Machine<Published> {
        //NOTE: we can access the state data that the previous state transitioned with
        let _draft_with_comment: &DraftWithComment = &self.state_data;
        self.transition()
    }
}

fn main() {}
