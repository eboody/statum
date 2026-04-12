#![allow(dead_code)]

use statum::{machine, state, transition, Branch};

#[state]
enum CheckoutState {
    Draft,
    ReadyToPay(u64),
    Paid,
}

#[machine]
struct Checkout<CheckoutState> {
    id: u64,
}

#[transition]
impl Checkout<Draft> {
    fn review(self, total_cents: u64) -> Checkout<ReadyToPay> {
        self.transition_with(total_cents)
    }
}

#[transition]
impl Checkout<ReadyToPay> {
    fn settle(self, paid: bool) -> ::statum::Branch<Checkout<Paid>, Checkout<Draft>> {
        if paid {
            Branch::First(self.transition())
        } else {
            Branch::Second(self.transition())
        }
    }
}

#[test]
fn core_state_machine_transition_surface_compiles_without_optional_features() {
    let ready = Checkout::<Draft>::builder().id(7).build().review(4_200);

    assert_eq!(ready.id, 7);
    assert_eq!(ready.state_data, 4_200);

    match ready.settle(true) {
        Branch::First(paid) => assert_eq!(paid.id, 7),
        Branch::Second(_) => panic!("expected paid branch"),
    }
}
