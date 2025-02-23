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
    fn into_in_review(self) -> Machine<InReview> {
        //NOTE: we use the transition method to move to the next state
        self.transition()
    }
}

#[transition]
impl Machine<InReview> {
    fn into_published(self) -> Machine<Published> {
        self.transition()
    }
}

fn main() {
    // we use the builder pattern to construct a new machine
    let machine = Machine::<Draft>::builder().build();

    // machine is now Machine<InReview>
    let machine = machine.into_in_review();

    // machine is now Machine<Published>
    let _machine = machine.into_published();
}
