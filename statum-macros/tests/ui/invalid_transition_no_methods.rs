use statum::{machine, state, transition};

#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<A> {}
