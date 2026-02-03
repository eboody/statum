extern crate statum;
use statum::state;

#[state]
enum GenericState<'a, T> {
    Draft(&'a T),
    InProgress(T),
}
