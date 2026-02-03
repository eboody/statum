extern crate statum;
use statum::{machine, state};

#[state]
enum MachineState {
    Ready,
}

#[machine]
struct Machine<S: Clone> {
    client: String,
}
