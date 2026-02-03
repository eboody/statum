extern crate statum;
use statum::state;

#[state]
enum BadState {
    Draft(u32, u32),
}
