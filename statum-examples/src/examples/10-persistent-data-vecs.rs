use statum::{machine, state, validators};

#[state]
enum State {
    Draft(Article),
    InReview,
    Published,
}

#[machine]
struct Machine<State> {
    client: String,
}

#[derive(Debug, PartialEq)]
enum Status {
    Draft,
    InReview,
    Published,
}

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
        println!("Machines client: {}", client);
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

pub async fn run() {
    let articles: Vec<Article> = pretend_db_call().await.unwrap();

    //NOTE: the builder is async because we have an async validator
    let machine_super_states = articles
        .machines_builder()
        .client("client".to_string())
        .build()
        .await;

    //NOTE: im throwing away the errors here, but in a real application you would want to handle
    //them
    let machine_super_states: Vec<MachineSuperState> = machine_super_states
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    for machine in machine_super_states {
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

    //OUTPUT:
    //_machine is Machine<Draft>
    //_machine is Machine<InReview>
    //_machine is Machine<Published>
    //_machine is Machine<Draft>
    //_machine is Machine<InReview>
}

async fn pretend_db_call() -> Result<Vec<Article>, statum::Error> {
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
        Article {
            status: Status::Draft,
        },
        Article {
            status: Status::InReview,
        },
    ];
    Ok(articles.into())
}
