#![allow(dead_code)]

use statum_graph::{CodebaseDoc, CodebaseDocError};

mod alpha {
    pub mod shared {
        use statum::{machine, state};

        #[state]
        pub enum State {
            Running,
        }

        #[machine]
        pub struct Machine<State> {}
    }
}

mod beta {
    pub mod shared {
        use statum::{machine, state};

        #[state]
        pub enum State {
            Running,
        }

        #[machine]
        pub struct Machine<State> {}
    }
}

mod holder {
    use super::alpha::shared;
    use statum::{machine, state};

    #[state]
    pub enum State {
        Uses(shared::Machine<shared::Running>),
    }

    #[machine]
    pub struct Machine<State> {}
}

#[test]
fn ambiguous_static_links_fail_closed() {
    let err = CodebaseDoc::linked().expect_err("ambiguous link should fail");

    assert_eq!(
        err,
        CodebaseDocError::AmbiguousStaticLink {
            machine: "codebase_ambiguity::holder::Machine",
            state: "Uses",
            field_name: None,
            target_machine_path: "shared::Machine".to_owned(),
            target_state: "Running",
        }
    );
}
