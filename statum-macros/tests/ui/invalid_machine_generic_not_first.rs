extern crate statum;
use statum::{machine, state};

#[state]
enum FooState {
    Start,
}

#[machine]
struct BadMachine<T, FooState> {
    value: T,
}
