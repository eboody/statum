#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::machine;
use bon::builder as _;

#[machine]
enum NotAStruct {
    Variant,
}