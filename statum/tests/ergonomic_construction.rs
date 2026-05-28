#![allow(dead_code)]

use statum::{machine, state, transition, validators};

#[state]
enum ArticleState {
    Draft,
    Review(String),
    Ready { reviewer: String, priority: u8 },
}

#[machine]
struct Article<ArticleState> {
    id: u64,
}

#[transition]
impl Article<Draft> {
    fn submit(self, reviewer: String) -> Article<Review> {
        self.transition_with(reviewer)
    }
}

struct ArticleRow {
    state: &'static str,
}

#[validators(Article)]
impl ArticleRow {
    fn is_draft(&self) -> statum::Result<()> {
        if self.state == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_review(&self) -> statum::Result<String> {
        if self.state == "review" {
            Ok("persisted-reviewer".to_owned())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_ready(&self) -> statum::Result<ReadyData> {
        if self.state == "ready" {
            Ok(ReadyData {
                reviewer: "persisted-ready".to_owned(),
                priority: 9,
            })
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

#[test]
fn builder_first_machine_keeps_rebuild_helpers() {
    let draft = Article::<Draft>::builder().id(1).build();
    assert_eq!(draft.id, 1);

    let review = draft.submit("alice".to_owned());
    assert_eq!(review.id, 1);
    assert_eq!(review.state_data, "alice");

    let review_direct = Article::<Review>::builder()
        .state_data("bob".to_owned())
        .id(2)
        .build();
    assert_eq!(review_direct.state_data, "bob");

    let ready = Article::<Ready>::builder()
        .state_data(ReadyData {
            reviewer: "carol".to_owned(),
            priority: 4,
        })
        .id(3)
        .build();
    assert_eq!(ready.id, 3);
    assert_eq!(ready.state_data.reviewer, "carol");
    assert_eq!(ready.state_data.priority, 4);

    let ready_via_state_data = Article::<Ready>::builder()
        .state_data(ReadyData {
            reviewer: "dave".to_owned(),
            priority: 7,
        })
        .id(4)
        .build();
    assert_eq!(ready_via_state_data.state_data.reviewer, "dave");
    assert_eq!(ready_via_state_data.state_data.priority, 7);

    let rebuilt = Article::rebuild(&ArticleRow { state: "ready" })
        .id(6)
        .build()
        .unwrap();
    match rebuilt {
        article::SomeState::Ready(machine) => {
            assert_eq!(machine.id, 6);
            assert_eq!(machine.state_data.reviewer, "persisted-ready");
            assert_eq!(machine.state_data.priority, 9);
        }
        _ => panic!("expected ready state"),
    }

    #[cfg(feature = "rebuild-batch")]
    {
        let rebuilt_many = Article::rebuild_many(vec![
            ArticleRow { state: "draft" },
            ArticleRow { state: "review" },
        ])
        .id(7)
        .build();
        assert_eq!(rebuilt_many.len(), 2);
        assert!(matches!(
            rebuilt_many[0].as_ref().unwrap(),
            article::SomeState::Draft(machine) if machine.id == 7
        ));
        assert!(matches!(
            rebuilt_many[1].as_ref().unwrap(),
            article::SomeState::Review(machine) if machine.state_data == "persisted-reviewer"
        ));
    }
}
