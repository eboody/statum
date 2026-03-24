#![allow(dead_code)]

use statum::{machine, state, transition, validators, MachineIntrospection};

#[state]
enum ArticleState {
    DraftNamed { title: String, version: u32 },
    ReviewNamed { reviewer: String, priority: u8 },
    PublishedNamed,
}

#[machine]
struct ArticleMachine<ArticleState> {
    id: u64,
}

#[transition]
impl ArticleMachine<DraftNamed> {
    fn submit(self, reviewer: &str) -> ArticleMachine<ReviewNamed> {
        self.transition_map(|draft| ReviewNamedData {
            reviewer: reviewer.to_owned(),
            priority: draft.version as u8,
        })
    }
}

#[transition]
impl ArticleMachine<ReviewNamed> {
    fn publish(self) -> ArticleMachine<PublishedNamed> {
        self.transition()
    }
}

struct ArticleRow {
    state: &'static str,
    title: &'static str,
    version: u32,
}

#[validators(ArticleMachine)]
impl ArticleRow {
    fn is_draft_named(&self) -> statum::Result<DraftNamedData> {
        let _ = id;
        if self.state == "draft" {
            Ok(DraftNamedData {
                title: self.title.to_owned(),
                version: self.version,
            })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_review_named(&self) -> statum::Result<ReviewNamedData> {
        let _ = id;
        Err(statum::Error::InvalidState)
    }

    fn is_published_named(&self) -> statum::Result<()> {
        let _ = id;
        Err(statum::Error::InvalidState)
    }
}

#[test]
fn named_field_states_work_with_transitions_validators_and_introspection() {
    let draft = ArticleMachine::<DraftNamed>::builder()
        .id(7)
        .state_data(DraftNamedData {
            title: "spec".to_owned(),
            version: 4,
        })
        .build();
    let review = draft.submit("alice");

    assert_eq!(review.id, 7);
    assert_eq!(review.state_data.reviewer, "alice");
    assert_eq!(review.state_data.priority, 4);
    let _published = review.publish();

    let graph = <ArticleMachine<DraftNamed> as MachineIntrospection>::GRAPH;
    assert!(
        graph
            .state(article_machine::StateId::DraftNamed)
            .unwrap()
            .has_data
    );
    assert!(
        graph
            .state(article_machine::StateId::ReviewNamed)
            .unwrap()
            .has_data
    );
    assert_eq!(
        graph
            .legal_targets(ArticleMachine::<DraftNamed>::SUBMIT)
            .unwrap(),
        &[article_machine::StateId::ReviewNamed]
    );

    let rebuilt = ArticleRow {
        state: "draft",
        title: "persisted",
        version: 9,
    }
    .into_machine()
    .id(11)
    .build()
    .unwrap();

    match rebuilt {
        article_machine::SomeState::DraftNamed(machine) => {
            assert_eq!(machine.id, 11);
            assert_eq!(machine.state_data.title, "persisted");
            assert_eq!(machine.state_data.version, 9);
        }
        _ => panic!("expected draft state"),
    }
}
