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

#[derive(Debug, PartialEq, Clone)]
enum Status {
    Draft,
    InReview,
    Published,
}

#[derive(Debug, Clone)]
struct Article {
    status: Status,
}

#[validators(Machine)]
impl Article {
    pub async fn is_draft(&self) -> Result<Article, statum::Error> {
        //NOTE: we have access to references of all of the machine's fields!
        //this way if we need to make, for example, network requests as a part of the validation
        //we can do that 🧙🪄

        let is_valid = pretend_validation_call(client).await?;

        if is_valid && self.status == Status::Draft {
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

async fn pretend_validation_call(_client: &str) -> Result<bool, statum::Error> {
    Ok(true)
}

pub async fn run() {
    let article = Article {
        status: Status::Draft,
    };

    let my_client = "my_client".to_string();

    //NOTE: machine_builder gives us MachineSuperState, an enum that represents all possible states
    // and their respective machines

    let machine_super_state: MachineSuperState = article
        .machine_builder()
        .client(my_client)
        .build()
        .await
        .unwrap();

    //NOTE: because MachineSuperState is just an enum, we can match on it however we want to get the specific machine
    match machine_super_state {
        MachineSuperState::Draft(_machine) => println!("do thing with Machine<Draft>"),
        MachineSuperState::InReview(_machine) => println!("do thing with Machine<InReview>"),
        MachineSuperState::Published(_machine) => println!("do thing with Machine<Published>"),
    }

    // Output:
    // Machines client in is_draft validator method: my_client
    // do thing with Machine<Draft>
}
