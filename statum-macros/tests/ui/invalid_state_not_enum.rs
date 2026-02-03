#![allow(unused_imports)]
use statum_macros::state;

#[state]
struct NotAnEnum {
    value: u32,
}