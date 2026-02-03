extern crate statum;
use statum::machine;

#[machine]
struct Machine {
    client: String,
}
