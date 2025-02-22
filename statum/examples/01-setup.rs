use statum::{machine, state};

#[state]
enum State {
    Draft,
    InReview,
    Published,
}

#[machine]
struct Machine<State> {}

fn main() {
    let _machine = Machine::<Draft>::builder().build();
}
