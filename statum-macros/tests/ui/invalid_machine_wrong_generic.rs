extern crate statum_macros;
use statum_macros::machine;

#[machine]
struct Machine<S: Clone> {
    // ❌ Should be
    client: String,
}
