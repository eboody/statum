use statum::{machine, state};

#[state]
enum State {
    Draft,
    InReview,
    Published,
}

#[machine]
struct Machine<State> {}

pub fn run() {
    let _machine = Machine::<Draft>::builder().build();
}
