#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::Error;
pub use bon;
use statum_macros::{machine, state, transition, validators};
use bon::builder as _;

mod simple {
    use super::*;

    #[state]
    enum State {
        A,
        B,
    }

    #[machine]
    struct Machine<State> {}

    #[transition]
    impl Machine<A> {
        fn to_b(self) -> Machine<B> {
            self.transition()
        }
    }
}

mod data_state {
    use super::*;

    #[state]
    enum State {
        Draft(ReviewData),
        Published,
    }

    struct ReviewData {
        reviewer: String,
    }

    #[machine]
    struct Machine<State> {
        id: u64,
    }

    #[transition]
    impl Machine<Draft> {
        fn publish(self) -> Machine<Published> {
            self.transition()
        }
    }
}

mod wrappers_option {
    use super::*;

    #[state]
    enum State {
        X,
        Y,
    }

    #[machine]
    struct Machine<State> {}

    #[transition]
    impl Machine<X> {
        fn to_y_option(self) -> Option<Machine<Y>> {
            Some(self.transition())
        }
    }
}

mod wrappers_result {
    use super::*;

    #[state]
    enum State {
        X,
        Y,
    }

    #[machine]
    struct Machine<State> {}

    #[transition]
    impl Machine<X> {
        fn to_y_result(self) -> Result<Machine<Y>, statum_core::Error> {
            Ok(self.transition())
        }
    }
}

mod validators_sync {
    use super::*;

    #[state]
    enum State {
        Draft,
        InReview(ReviewData),
    }

    struct ReviewData {
        reviewer: String,
    }

    #[machine]
    struct Machine<State> {
        tenant: String,
    }

    struct Row {
        status: &'static str,
    }

    #[validators(Machine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            if self.status == "draft" { Ok(()) } else { Err(statum_core::Error::InvalidState) }
        }

        fn is_in_review(&self) -> Result<ReviewData, statum_core::Error> {
            if self.status == "review" {
                Ok(ReviewData { reviewer: "a".to_string() })
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }
}

fn main() {}