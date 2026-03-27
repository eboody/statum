#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum CheckoutState {
    EmptyCart,
    ReadyToPay(OrderDraft),
    Paid,
}

#[derive(Clone)]
struct OrderDraft {
    total_cents: u64,
}

#[machine]
struct Checkout<CheckoutState> {
    id: String,
}

#[transition]
impl Checkout<EmptyCart> {
    fn review(self, total_cents: u64) -> Checkout<ReadyToPay> {
        self.transition_with(OrderDraft { total_cents })
    }
}

#[transition]
impl Checkout<ReadyToPay> {
    fn pay(self) -> Checkout<Paid> {
        self.transition()
    }
}

fn main() {
    let cart = Checkout::<EmptyCart>::builder()
        .id("order-1".to_owned())
        .build();

    let ready = cart.review(4200);
    assert_eq!(ready.state_data.total_cents, 4200);

    let _paid = ready.pay();
}
