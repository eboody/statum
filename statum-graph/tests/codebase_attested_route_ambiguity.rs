#![allow(dead_code)]

use statum_graph::{CodebaseDoc, CodebaseDocError};

mod workflow {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Review,
        Published,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Draft> {
        fn submit(self) -> Machine<Review> {
            self.transition()
        }
    }

    #[transition]
    impl Machine<Review> {
        fn submit(self) -> Machine<Published> {
            self.transition()
        }
    }
}

#[test]
fn duplicate_attested_route_names_fail_closed() {
    let err = CodebaseDoc::linked().expect_err("duplicate attested route should fail");

    assert_eq!(
        err,
        CodebaseDocError::DuplicateViaRoute {
            via_module_path: "codebase_attested_route_ambiguity::workflow::machine::via",
            route_name: "Submit",
        }
    );
}
