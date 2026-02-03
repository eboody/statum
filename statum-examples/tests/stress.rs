use statum::{machine, state, transition, validators};

mod unit_and_data_transitions {
    use super::*;

    #[state]
    enum State {
        Draft,
        InReview(ReviewData),
        Published,
    }

    #[derive(Clone)]
    struct ReviewData {
        reviewer: String,
    }

    #[machine]
    struct Machine<State> {
        id: u64,
    }

    #[transition]
    impl Machine<Draft> {
        fn submit(self, reviewer: String) -> Machine<InReview> {
            self.transition_with(ReviewData { reviewer })
        }
    }

    #[transition]
    impl Machine<InReview> {
        fn publish(self) -> Machine<Published> {
            self.transition()
        }
    }

    pub fn run() {
        let machine = Machine::<Draft>::builder().id(1).build();
        let machine = machine.submit("sam".to_string());
        let _reviewer = machine.state_data.reviewer.as_str();
        let _machine = machine.publish();
    }
}

mod wrapped_option_transition {
    use super::*;

    #[state]
    enum State {
        A,
        B,
    }

    #[machine]
    struct Machine<State> {
        name: String,
    }

    #[transition]
    impl Machine<A> {
        fn to_b_option(self) -> Option<Machine<B>> {
            Some(self.transition())
        }
    }

    pub fn run() {
        let machine = Machine::<A>::builder().name("m".to_string()).build();
        let _ = machine.to_b_option();
    }
}

mod wrapped_result_transition {
    use super::*;

    #[state]
    enum State {
        A,
        B,
    }

    #[machine]
    struct Machine<State> {
        name: String,
    }

    #[transition]
    impl Machine<A> {
        fn to_b_result(self) -> Result<Machine<B>, statum::Error> {
            Ok(self.transition())
        }
    }

    pub fn run() {
        let machine = Machine::<A>::builder().name("m".to_string()).build();
        let _ = machine.to_b_result();
    }
}

mod validators_sync_and_async {
    use super::*;

    #[state]
    enum State {
        Draft,
        InReview(ReviewData),
        Published,
    }

    #[derive(Clone)]
    struct ReviewData {
        reviewer: String,
    }

    #[machine]
    struct Machine<State> {
        tenant: String,
    }

    #[derive(Clone)]
    struct Row {
        status: &'static str,
    }

    #[validators(Machine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum::Error> {
            if self.status == "draft" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }

        async fn is_in_review(&self) -> Result<ReviewData, statum::Error> {
            if self.status == "review" {
                Ok(ReviewData { reviewer: "a".to_string() })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> Result<(), statum::Error> {
            if self.status == "published" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }
    }

    pub async fn run() {
        let row = Row { status: "review" };
        let state = row
            .machine_builder()
            .tenant("t".to_string())
            .build()
            .await
            .unwrap();

        match state {
            MachineSuperState::InReview(machine) => {
                let _reviewer = machine.state_data.reviewer.as_str();
            }
            _ => panic!("unexpected state"),
        }
    }
}

mod hierarchical_state_data {
    use super::*;

    pub mod sub {
        use super::*;

        #[state]
        pub enum State {
            Idle,
            Running,
        }

        #[machine]
        pub struct Machine<State> {}
    }

    pub mod parent {
        use super::*;

        #[state]
        pub enum State {
            NotStarted,
            InProgress(crate::hierarchical_state_data::sub::Machine<crate::hierarchical_state_data::sub::Running>),
            Done,
        }

        #[machine]
        pub struct Machine<State> {}

        #[transition]
        impl Machine<NotStarted> {
            pub fn start(
                self,
                sub: crate::hierarchical_state_data::sub::Machine<crate::hierarchical_state_data::sub::Running>,
            ) -> Machine<InProgress> {
                self.transition_with(sub)
            }
        }

        #[transition]
        impl Machine<InProgress> {
            pub fn finish(self) -> Machine<Done> {
                self.transition()
            }
        }
    }

    pub fn run() {
        let sub = sub::Machine::<sub::Running>::builder().build();
        let parent = parent::Machine::<parent::NotStarted>::builder().build();
        let parent = parent.start(sub);
        let _parent = parent.finish();
    }
}

#[test]
fn stress_unit_and_data_transitions() {
    unit_and_data_transitions::run();
}

#[test]
fn stress_wrapped_option() {
    wrapped_option_transition::run();
}

#[test]
fn stress_wrapped_result() {
    wrapped_result_transition::run();
}

#[tokio::test]
async fn stress_validators_sync_and_async() {
    validators_sync_and_async::run().await;
}

#[test]
fn stress_hierarchical_state_data() {
    hierarchical_state_data::run();
}
