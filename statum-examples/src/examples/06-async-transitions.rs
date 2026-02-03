use statum::{machine, state, transition};

#[state]
enum State {
    Draft,
    InReview,
    Published,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    //NOTE: this is an async transition
    async fn to_in_review(self) -> Machine<InReview> {
        self.transition()
    }
}

pub async fn run() {
    let machine = Machine::<Draft>::builder().build();

    // NOTE: we're awaiting here
    let _machine = machine.to_in_review().await;
}
