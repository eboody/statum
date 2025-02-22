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
    pub fn is_draft(&self) -> Result<Article, statum::Error> {
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

fn main() {
    let article = Article {
        status: Status::Draft,
    };

    let machine_super_state = article.machine_builder().build().unwrap();

    match machine_super_state {
        MachineSuperState::Draft(_machine) => println!("do thing with Machine<Draft>"),
        MachineSuperState::InReview(_machine) => println!("do thing with Machine<InReview>"),
        MachineSuperState::Published(_machine) => println!("do thing with Machine<Published>"),
    }

    if article.is_draft().is_ok() {
        let _machine = Machine::<Draft>::builder().state_data(article).build();
    }
}
