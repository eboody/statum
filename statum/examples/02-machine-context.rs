use statum::{machine, state};

#[state]
enum State {
    Draft,
    InReview,
    Published,
}

#[machine]
struct Machine<State> {
    // NOTE: the fields of your machine should be the context necessary for working with your machine
    client: String,
    db_pool: String,
}

fn main() {
    let my_client = "Pretend this is some client".to_owned();
    let my_db_pool = "Pretend this is a db pool".to_owned();

    let _machine = Machine::<Draft>::builder()
        .client(my_client)
        .db_pool(my_db_pool)
        .build();
}
