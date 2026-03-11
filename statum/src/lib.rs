//! Statum provides ergonomic typestate-builder and lifecycle APIs in Rust.
//!
//! It builds on finite-state modeling and validates legal transitions at compile time.
//! The main macros are:
//!
//! - [`state`](macro@state) for defining lifecycle states.
//! - [`machine`](macro@machine) for defining the state-carrying machine/builder.
//! - [`transition`](macro@transition) for validating transition impl signatures.
//! - [`validators`](macro@validators) for rehydrating typed machines from persisted data.
//!
//! # Quick Example
//!
//! ```
//! use statum::{machine, state, transition};
//!
//! #[state]
//! enum CheckoutState {
//!     EmptyCart,
//!     ReadyToPay(OrderDraft),
//!     Paid,
//! }
//!
//! #[derive(Clone)]
//! struct OrderDraft {
//!     total_cents: u64,
//! }
//!
//! #[machine]
//! struct Checkout<CheckoutState> {
//!     id: String,
//! }
//!
//! #[transition]
//! impl Checkout<EmptyCart> {
//!     fn review(self, total_cents: u64) -> Checkout<ReadyToPay> {
//!         self.transition_with(OrderDraft { total_cents })
//!     }
//! }
//!
//! #[transition]
//! impl Checkout<ReadyToPay> {
//!     fn pay(self) -> Checkout<Paid> {
//!         self.transition()
//!     }
//! }
//!
//! fn main() {
//!     let cart = Checkout::<EmptyCart>::builder()
//!         .id("order-1".to_owned())
//!         .build();
//!
//!     let ready = cart.review(4200);
//!     assert_eq!(ready.state_data.total_cents, 4200);
//!
//!     let _paid = ready.pay();
//! }
//! ```
//!
//! # Compile-Time Gating
//!
//! Methods only exist on states where you define them.
//!
//! ```compile_fail
//! use statum::{machine, state};
//!
//! #[state]
//! enum LightState {
//!     Off,
//!     On,
//! }
//!
//! #[machine]
//! struct Light<LightState> {}
//!
//! fn main() {
//!     let light = Light::<Off>::builder().build();
//!     let _ = light.switch_off(); // no such method on Light<Off>
//! }
//! ```

pub use bon;
pub use statum_core::projection;
pub use statum_core::*;
pub use statum_macros::*;
