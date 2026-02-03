extern crate statum;
use statum::state;

#[state]
struct NotAnEnum {
    value: u32,
}
