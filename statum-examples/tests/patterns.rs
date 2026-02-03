use statum::{machine, state, transition, validators};

mod rehydration_with_fetch {
    use super::*;

    #[state]
    enum State {
        Draft,
        InReview(ReviewData),
    }

    #[derive(Clone)]
    struct ReviewData {
        reviewer: String,
    }

    #[machine]
    struct Machine<State> {
        client: String,
    }

    struct Row {
        status: &'static str,
    }

    fn fetch_reviewer(client: &str) -> String {
        format!("reviewer:{client}")
    }

    #[validators(Machine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum::Error> {
            if self.status == "draft" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }

        fn is_in_review(&self) -> Result<ReviewData, statum::Error> {
            if self.status == "review" {
                Ok(ReviewData { reviewer: fetch_reviewer(client) })
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn run() {
        let row = Row { status: "review" };
        let machine = row
            .machine_builder()
            .client("acme".to_string())
            .build()
            .unwrap();

        match machine {
            MachineSuperState::InReview(m) => {
                assert_eq!(m.state_data.reviewer.as_str(), "reviewer:acme");
            }
            _ => panic!("unexpected state"),
        }
    }
}

mod event_driven_transitions {
    use super::*;

    #[state]
    enum State {
        Init,
        Next,
        Other,
    }

    #[machine]
    struct Machine<State> {}

    enum Event {
        Go,
        Alternative,
    }

    enum Decision {
        Next(Machine<Next>),
        Other(Machine<Other>),
    }

    #[transition]
    impl Machine<Init> {
        fn to_next(self) -> Machine<Next> {
            self.transition()
        }

        fn to_other(self) -> Machine<Other> {
            self.transition()
        }
    }

    impl Machine<Init> {
        fn handle_event(self, event: Event) -> Decision {
            match event {
                Event::Go => Decision::Next(self.to_next()),
                Event::Alternative => Decision::Other(self.to_other()),
            }
        }
    }

    pub fn run() {
        let machine = Machine::<Init>::builder().build();
        match machine.handle_event(Event::Go) {
            Decision::Next(_) => {}
            Decision::Other(_) => panic!("wrong path"),
        }

        let machine = Machine::<Init>::builder().build();
        match machine.handle_event(Event::Alternative) {
            Decision::Other(_) => {}
            Decision::Next(_) => panic!("wrong path"),
        }
    }
}

mod guarded_transitions {
    use super::*;

    #[state]
    enum State {
        Pending,
        Active,
    }

    #[machine]
    struct Machine<State> {
        allowed: bool,
    }

    #[transition]
    impl Machine<Pending> {
        fn activate(self) -> Machine<Active> {
            self.transition()
        }
    }

    impl Machine<Pending> {
        fn can_activate(&self) -> bool {
            self.allowed
        }

        fn try_activate(self) -> Result<Machine<Active>, statum::Error> {
            if self.can_activate() {
                Ok(self.activate())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn run() {
        let machine = Machine::<Pending>::builder().allowed(true).build();
        let _ = machine.try_activate().unwrap();
    }
}

mod state_snapshots {
    use super::*;

    #[state]
    enum State {
        Draft(DraftData),
        Published(PublishData),
    }

    #[derive(Clone)]
    struct DraftData {
        title: String,
    }

    #[derive(Clone)]
    struct PublishData {
        previous: DraftData,
    }

    #[machine]
    struct Machine<State> {}

    #[transition]
    impl Machine<Draft> {
        fn publish(self) -> Machine<Published> {
            let previous = self.state_data.clone();
            self.transition_with(PublishData { previous })
        }
    }

    pub fn run() {
        let draft = DraftData { title: "doc".to_string() };
        let machine = Machine::<Draft>::builder().state_data(draft).build();
        let published = machine.publish();
        assert_eq!(published.state_data.previous.title.as_str(), "doc");
    }
}

mod async_side_effects {
    use super::*;

    #[state]
    enum State {
        Queued,
        Running,
    }

    #[machine]
    struct Machine<State> {}

    #[transition]
    impl Machine<Queued> {
        fn start(self) -> Machine<Running> {
            self.transition()
        }
    }

    impl Machine<Queued> {
        async fn start_with_effects(self) -> Machine<Running> {
            tokio::task::yield_now().await;
            self.start()
        }
    }

    pub async fn run() {
        let machine = Machine::<Queued>::builder().build();
        let _running = machine.start_with_effects().await;
    }
}

mod parallel_reconstruction {
    use super::*;

    #[state]
    enum State {
        Draft,
        Published,
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
        async fn is_draft(&self) -> Result<(), statum::Error> {
            if self.status == "draft" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }

        async fn is_published(&self) -> Result<(), statum::Error> {
            if self.status == "published" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }
    }

    pub async fn run() {
        let rows = vec![Row { status: "draft" }, Row { status: "published" }];
        let results = rows
            .machines_builder()
            .tenant("t".to_string())
            .build()
            .await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }
}

mod type_erased_storage {
    use super::*;

    #[state]
    enum State {
        A,
        B,
    }

    #[machine]
    struct Machine<State> {}

    struct Row {
        status: &'static str,
    }

    #[validators(Machine)]
    impl Row {
        fn is_a(&self) -> Result<(), statum::Error> {
            if self.status == "a" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }

        fn is_b(&self) -> Result<(), statum::Error> {
            if self.status == "b" { Ok(()) } else { Err(statum::Error::InvalidState) }
        }
    }

    #[transition]
    impl Machine<A> {
        fn to_b(self) -> Machine<B> {
            self.transition()
        }
    }

    pub fn run() {
        let row = Row { status: "a" };
        let machine = row.machine_builder().build().unwrap();
        let items: Vec<MachineSuperState> = vec![machine];

        for item in items {
            match item {
                MachineSuperState::A(machine) => {
                    let _ = machine.to_b();
                }
                MachineSuperState::B(_machine) => {}
            }
        }
    }
}

#[test]
fn patterns_rehydration_with_fetch() {
    rehydration_with_fetch::run();
}

#[test]
fn patterns_event_driven_transitions() {
    event_driven_transitions::run();
}

#[test]
fn patterns_guarded_transitions() {
    guarded_transitions::run();
}

#[test]
fn patterns_state_snapshots() {
    state_snapshots::run();
}

#[tokio::test]
async fn patterns_async_side_effects() {
    async_side_effects::run().await;
}

#[tokio::test]
async fn patterns_parallel_reconstruction() {
    parallel_reconstruction::run().await;
}

#[test]
fn patterns_type_erased_storage() {
    type_erased_storage::run();
}
