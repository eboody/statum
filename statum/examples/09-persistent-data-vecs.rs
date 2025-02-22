use statum::{machine, state, validators};

#[state]
enum State {
    Draft(Article),
    InReview,
    Published,
}

#[machine]
struct Machine<State> {}

#[derive(Debug, PartialEq)]
enum Status {
    Draft,
    InReview,
    Published,
}

#[derive(Debug)]
struct Article {
    status: Status,
}

#[validators(Machine)]
impl Article {
    pub async fn is_draft(&self) -> Result<Article, statum::Error> {
        if self.status == Status::Draft {
            Ok(Article {
                status: Status::Draft,
            })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    pub fn is_in_review(&self) -> Result<(), statum::Error> {
        if self.status == Status::InReview {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    pub fn is_published(&self) -> Result<(), statum::Error> {
        if self.status == Status::Published {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

#[tokio::main]
async fn main() {
    let articles = [
        Article {
            status: Status::Draft,
        },
        Article {
            status: Status::InReview,
        },
        Article {
            status: Status::Published,
        },
    ];

    //NOTE: im throwing away the errors here, but in a real application you would want to handle
    //them
    let machines: Vec<MachineSuperState> = articles
        .build_machines()
        .await
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    for machine in machines {
        match machine {
            MachineSuperState::Draft(_machine) => {
                println!("_machine is Machine<Draft>");
            }
            MachineSuperState::InReview(_machine) => {
                println!("_machine is Machine<InReview>");
            }
            MachineSuperState::Published(_machine) => {
                println!("_machine is Machine<Published>");
            }
        }
    }
}
