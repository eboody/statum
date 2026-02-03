#![allow(unused_imports)]
use statum_macros::state;

#[state]
enum BadState {
    Draft(u32, u32),
}