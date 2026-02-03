extern crate statum;
use statum::state;
#[state]
enum BadState {
    Draft { version: u32 },
}
