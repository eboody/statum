extern crate statum_macros_two as statum_macros;
use statum_macros::machine;

#[machine]
struct Machine {
    // ❌ Missing `<S: State>`
    client: String,
}
