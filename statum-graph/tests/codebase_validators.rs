#![allow(dead_code)]

use statum_graph::CodebaseDoc;

mod cfg_surface {
    use statum::{machine, state, validators, Error};

    #[state]
    pub enum State {
        Draft,
        Done,
    }

    #[machine]
    pub struct Machine<State> {}

    pub struct VisibleRow {
        pub done: bool,
    }

    /// Rebuilds cfg-surface machines from visible persisted rows.
    #[validators(Machine)]
    impl VisibleRow {
        fn is_draft(&self) -> statum::Result<()> {
            if !self.done {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_done(&self) -> statum::Result<()> {
            if self.done {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }
    }

    pub struct HiddenRow;

    /// Hidden validator docs should not appear in the active build.
    #[cfg(any())]
    #[validators(Machine)]
    impl HiddenRow {
        fn is_draft(&self) -> statum::Result<()> {
            Ok(())
        }

        fn is_done(&self) -> statum::Result<()> {
            Ok(())
        }
    }
}

mod macro_surface {
    use statum::{machine, state, validators, Error};

    #[state]
    pub enum State {
        Draft,
        Done,
    }

    #[machine]
    pub struct Machine<State> {}

    pub struct MacroRow {
        pub done: bool,
    }

    macro_rules! define_validators {
        () => {
            /// Rebuilds macro-surface machines from macro-generated rows.
            #[validators(Machine)]
            impl MacroRow {
                fn is_draft(&self) -> statum::Result<()> {
                    if !self.done {
                        Ok(())
                    } else {
                        Err(Error::InvalidState)
                    }
                }

                fn is_done(&self) -> statum::Result<()> {
                    if self.done {
                        Ok(())
                    } else {
                        Err(Error::InvalidState)
                    }
                }
            }
        };
    }

    define_validators!();
}

#[test]
fn linked_codebase_uses_compiled_validator_impl_surfaces() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let cfg_machine = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("cfg_surface::Machine"))
        .expect("cfg machine");
    assert_eq!(cfg_machine.validator_entries.len(), 1);
    assert_eq!(
        cfg_machine.validator_entries[0].source_type_display,
        "VisibleRow"
    );
    assert_eq!(cfg_machine.validator_entries[0].target_states, vec![0, 1]);
    assert_eq!(
        cfg_machine.validator_entries[0].docs,
        Some("Rebuilds cfg-surface machines from visible persisted rows.")
    );

    let macro_machine = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("macro_surface::Machine"))
        .expect("macro machine");
    assert_eq!(macro_machine.validator_entries.len(), 1);
    assert_eq!(
        macro_machine.validator_entries[0].source_type_display,
        "MacroRow"
    );
    assert_eq!(macro_machine.validator_entries[0].target_states, vec![0, 1]);
    assert_eq!(
        macro_machine.validator_entries[0].docs,
        Some("Rebuilds macro-surface machines from macro-generated rows.")
    );
}
