extern crate statum_macros_two as statum_macros;
use statum_macros::state;

#[state]
struct NotAnEnum {
    value: u32,
}
