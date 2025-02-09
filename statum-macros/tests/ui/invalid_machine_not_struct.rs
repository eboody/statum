extern crate statum_macros;
use statum_macros::machine;
#[machine]
enum NotAStruct {
    Variant,
}
