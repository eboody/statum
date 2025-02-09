extern crate statum_macros_two as statum_macros;
use statum_macros::machine;

#[machine]
struct Machine<T> {
    // ❌ `T` does not implement `State`
    client: String,
}
