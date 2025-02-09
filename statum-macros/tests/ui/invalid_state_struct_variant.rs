extern crate statum_macros;
use statum_macros::state;
#[state]
enum BadState {
    Draft { version: u32 },
}
