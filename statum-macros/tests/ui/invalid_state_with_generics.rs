#![allow(unused_imports)]
use statum_macros::state;

#[state]
enum GenericState<'a, T> {
    Draft(&'a T),
    InProgress(T),
}