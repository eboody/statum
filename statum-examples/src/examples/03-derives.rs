use statum::{machine, state};

//NOTE: the derives must be UNDER the state and machine macros
#[state]
#[derive(Clone, Debug)]
enum State {
    Draft,
    InReview,
    Published,
}

//NOTE: the derives must be UNDER the state and machine macros
#[machine]
#[derive(Clone, Debug)]
struct Machine<State> {}

pub fn run() {
    let machine = Machine::<Draft>::builder().build();

    let machine_clone = machine.clone();
    println!("Cloned machine: {:?}", machine_clone);
}
