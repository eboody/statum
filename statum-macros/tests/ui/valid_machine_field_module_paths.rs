#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};


use statum_macros::{machine, state, validators};


mod shared {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Text;
}

mod domain {
    pub mod chat {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct RoomId(pub u64);
    }
}

mod same_module_path_case {
    use super::*;

    mod support {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Text;
    }

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub title: support::Text,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &title;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    pub fn smoke() {
        let direct = WorkflowMachine::<Draft>::builder().title(support::Text).build();
        let _ = direct.title;

        let rebuilt = Row { status: "draft" }
            .into_machine()
            .title(support::Text)
            .build()
            .unwrap();
        match rebuilt {
            workflow_machine::SomeState::Draft(machine) => {
                let _ = machine.title;
            }
        }
    }
}

mod self_path_case {
    use super::*;

    mod support {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Text;
    }

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub title: self::support::Text,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &title;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    pub fn smoke() {
        let direct = WorkflowMachine::<Draft>::builder().title(support::Text).build();
        let _ = direct.title;

        let rebuilt = Row { status: "draft" }
            .into_machine()
            .title(support::Text)
            .build()
            .unwrap();
        match rebuilt {
            workflow_machine::SomeState::Draft(machine) => {
                let _ = machine.title;
            }
        }
    }
}

mod crate_path_case {
    use super::*;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub title: crate::shared::Text,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &title;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    pub fn smoke() {
        let direct = WorkflowMachine::<Draft>::builder().title(crate::shared::Text).build();
        let _ = direct.title;

        let rebuilt = Row { status: "draft" }
            .into_machine()
            .title(crate::shared::Text)
            .build()
            .unwrap();
        match rebuilt {
            workflow_machine::SomeState::Draft(machine) => {
                let _ = machine.title;
            }
        }
    }
}

mod imported_module_case {
    use super::*;
    use crate::domain::chat;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub room_id: chat::RoomId,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &room_id;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    pub fn smoke() {
        let direct = WorkflowMachine::<Draft>::builder()
            .room_id(domain::chat::RoomId(1))
            .build();
        let _ = direct.room_id.0;

        let rebuilt = Row { status: "draft" }
            .into_machine()
            .room_id(domain::chat::RoomId(2))
            .build()
            .unwrap();
        match rebuilt {
            workflow_machine::SomeState::Draft(machine) => {
                let _ = machine.room_id.0;
            }
        }
    }
}

mod renamed_module_case {
    use super::*;
    use crate::domain::chat as flow_chat;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub room_id: flow_chat::RoomId,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &room_id;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    pub fn smoke() {
        let direct = WorkflowMachine::<Draft>::builder()
            .room_id(domain::chat::RoomId(3))
            .build();
        let _ = direct.room_id.0;

        let rebuilt = Row { status: "draft" }
            .into_machine()
            .room_id(domain::chat::RoomId(4))
            .build()
            .unwrap();
        match rebuilt {
            workflow_machine::SomeState::Draft(machine) => {
                let _ = machine.room_id.0;
            }
        }
    }
}

mod super_path_case {
    use super::*;

    mod shared {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Text;
    }

    pub mod nested {
        use super::*;

        #[state]
        pub enum WorkflowState {
            Draft,
        }

        #[machine]
        pub struct WorkflowMachine<WorkflowState> {
            pub title: super::shared::Text,
        }

        pub struct Row {
            pub status: &'static str,
        }

        #[validators(WorkflowMachine)]
        impl Row {
            fn is_draft(&self) -> Result<(), statum_core::Error> {
                let _ = &title;
                if self.status == "draft" {
                    Ok(())
                } else {
                    Err(statum_core::Error::InvalidState)
                }
            }
        }

        pub fn smoke() {
            let direct = WorkflowMachine::<Draft>::builder().title(super::shared::Text).build();
            let _ = direct.title;

            let rebuilt = Row { status: "draft" }
                .into_machine()
                .title(super::shared::Text)
                .build()
                .unwrap();
            match rebuilt {
                workflow_machine::SomeState::Draft(machine) => {
                    let _ = machine.title;
                }
            }
        }
    }
}

fn main() {
    same_module_path_case::smoke();
    self_path_case::smoke();
    crate_path_case::smoke();
    imported_module_case::smoke();
    renamed_module_case::smoke();
    super_path_case::nested::smoke();
}
