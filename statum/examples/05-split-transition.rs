use statum::{machine, state, transition};

#[state]
#[derive(Clone)]
enum CheckoutState {
    Cart,
    PaymentPending(String),
    PaymentConfirmed,
    Shipped,
}

#[machine]
#[derive(Clone)]
struct OrderMachine<CheckoutState> {
    user_id: u64,
}

#[transition]
impl OrderMachine<Cart> {
    pub fn proceed_to_payment(self) -> OrderMachine<PaymentPending> {
        self.transition_with("txn_123".to_string())
    }
}

#[transition]
impl OrderMachine<PaymentPending> {
    pub fn confirm_payment(self) -> OrderMachine<PaymentConfirmed> {
        self.transition()
    }

    pub fn cancel_payment(self) -> OrderMachine<Cart> {
        self.transition()
    }
}

fn main() {
    let cart_machine = OrderMachine::<Cart>::builder().user_id(123).build();

    // 🔥 Works! Rust infers that `transition_with<String>()` should be called.
    let pending = cart_machine.proceed_to_payment();

    // 🔥 Works! Rust selects the correct `transition()` implementation.
    let _confirmed = pending.clone().confirm_payment();

    //// 🔥 Works! PaymentPending -> Cart also compiles fine.
    let _back_to_cart = pending.cancel_payment();
}
