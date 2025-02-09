extern crate statum_macros_two as statum_macros;
use statum_macros::state;

#[state]
enum EmptyState {} // ❌ No variants
