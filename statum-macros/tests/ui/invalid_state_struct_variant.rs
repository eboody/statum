#![allow(unused_imports)]
use statum_macros::state;

#[state]
enum BadState {
    Draft { version: u32 },
}