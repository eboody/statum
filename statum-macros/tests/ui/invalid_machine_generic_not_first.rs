#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::{machine, state};
use bon::builder as _;

#[state]
enum FooState {
    Start,
}

#[machine]
struct BadMachine<T, FooState> {
    value: T,
}