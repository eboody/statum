extern crate statum_macros;
use statum_macros::state;

#[state]
enum GenericState<'a, T> {
    Draft(&'a T),
    InProgress(T),
}
