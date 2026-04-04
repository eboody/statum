#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    Attested, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    MachineDescriptor, MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt,
    RebuildReport, StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum PaymentState {
    Authorized,
    Captured,
}

#[machine]
struct Payment<PaymentState> {}

#[transition]
impl Payment<Authorized> {
    fn capture(self) -> Payment<Captured> {
        self.transition()
    }
}

#[state]
enum FulfillmentState {
    ReadyToShip,
    Shipping,
    ReceiptQueued,
}

#[machine]
struct Fulfillment<FulfillmentState> {}

#[transition]
impl Fulfillment<ReadyToShip> {
    fn start_shipping(
        self,
        #[via(crate::payment::via::Capture)]
        payment: Payment<Captured>,
    ) -> Fulfillment<Shipping> {
        let _ = payment;
        self.transition()
    }

    fn queue_receipt_email(
        self,
        #[via(crate::payment::via::Capture)]
        payment: Payment<Captured>,
    ) -> Fulfillment<ReceiptQueued> {
        let _ = payment;
        self.transition()
    }
}

fn main() {
    let payment = Payment::<Authorized>::builder().build();
    let captured = payment.capture_and_attest();
    let fulfillment = Fulfillment::<ReadyToShip>::builder().build();
    let _shipping = fulfillment.from_capture(captured).start_shipping();

    let payment = Payment::<Authorized>::builder().build();
    let captured = payment.capture_and_attest();
    let fulfillment = Fulfillment::<ReadyToShip>::builder().build();
    let _queued = fulfillment
        .from_capture(captured)
        .queue_receipt_email();
}
