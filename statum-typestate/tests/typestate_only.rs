use statum::{machine, state, transition};

#[state]
enum ReviewState {
    Draft,
    Submitted(Submission),
    Approved,
    Rejected(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Submission {
    title: String,
}

#[machine]
struct Review<ReviewState> {
    id: u64,
}

#[transition]
impl Review<Draft> {
    fn submit(self, title: impl Into<String>) -> Review<Submitted> {
        self.transition_with(Submission {
            title: title.into(),
        })
    }
}

#[transition]
impl Review<Submitted> {
    fn decide(self, approve: bool) -> ::statum::Branch<Review<Approved>, Review<Rejected>> {
        if approve {
            ::statum::Branch::First(self.approve())
        } else {
            ::statum::Branch::Second(self.reject("needs revision".to_owned()))
        }
    }

    fn approve(self) -> Review<Approved> {
        self.transition()
    }

    fn reject(self, reason: String) -> Review<Rejected> {
        self.transition_with(reason)
    }
}

#[test]
fn supports_typestate_only_surface() {
    let review = Review::<Draft>::builder().id(41).build();
    let submitted = review.submit("RFC");
    assert_eq!(submitted.state_data.title, "RFC");

    let rejected = match submitted.decide(false) {
        ::statum::Branch::First(_) => panic!("expected rejection"),
        ::statum::Branch::Second(review) => review,
    };

    assert_eq!(rejected.id, 41);
    assert_eq!(rejected.state_data, "needs revision");
}
