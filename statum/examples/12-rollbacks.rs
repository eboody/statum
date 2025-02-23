use statum::{machine, state, transition};

mod transaction {
    use super::*;
    #[state]
    pub enum State {
        Pending,
        Confirmed,
        Reverted,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Pending> {
        pub fn confirm(self) -> Result<Machine<Confirmed>, Machine<Reverted>> {
            if true {
                Ok(self.transition())
            } else {
                Err(self.rollback())
            }
        }

        pub fn rollback(self) -> Machine<Reverted> {
            self.transition()
        }
    }
}

fn main() {
    let _machine = transaction::Machine::<transaction::Pending>::builder().build();

    let _machine = _machine.confirm();

    match _machine {
        Ok(_confirmed_machine) => println!("We have Machine<Confirmed>"),
        Err(_revered_machine) => println!("We have Machine<Reverted>"),
    }
}
