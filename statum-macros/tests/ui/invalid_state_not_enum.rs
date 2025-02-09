extern crate statum_macros;
use statum_macros::state;

#[state]
struct NotAnEnum {
    value: u32,
}
