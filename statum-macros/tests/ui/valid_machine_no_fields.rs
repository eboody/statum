#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::{machine, state};
use bon::builder as _;

#[state]
pub enum ToggleState {
    On,
    Off,
}

#[machine]
pub struct Switch<ToggleState>;

fn main() {
    let _: Switch<On> = Switch::<On>::builder().build();
}