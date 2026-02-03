extern crate statum;
use statum::{machine, state};

#[state]
#[derive(Debug)]
enum BuildState {
    Ready,
    Done,
}

#[machine]
#[derive(Debug, Clone)]
struct BuildMachine<BuildState> {
    name: String,
}
