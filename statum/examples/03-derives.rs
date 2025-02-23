use serde::{Deserialize, Serialize};
use statum::{machine, state};

//NOTE: the derives must be UNDER the state and machine macros
#[state]
#[derive(Clone, Serialize, Deserialize)]
enum State {
    Draft,
    InReview,
    Published,
}

//NOTE: the derives must be UNDER the state and machine macros
#[machine]
#[derive(Clone, Serialize, Deserialize)]
struct Machine<State> {}

fn main() {
    let machine = Machine::<Draft>::builder().build();

    let machine_clone = machine.clone();

    let json_machine = serde_json::to_string(&machine_clone).unwrap();

    println!("It's been successfully deserialized: {}", json_machine);

    let _deserialized_machine: Machine<Draft> = serde_json::from_str(&json_machine).unwrap();
}
