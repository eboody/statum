#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::{machine, state};
use bon::builder as _;

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