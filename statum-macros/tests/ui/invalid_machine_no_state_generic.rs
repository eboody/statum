extern crate statum_macros;
use statum_macros::machine;

#[machine]
struct Machine {
    client: String,
}
