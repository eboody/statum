extern crate statum_macros_two as statum_macros;
use statum_macros::state;

#[state]
enum BadState {
    Draft { version: u32 }, // ❌ Struct-like variant is not allowed
}
