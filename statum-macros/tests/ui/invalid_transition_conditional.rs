#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::{machine, state, transition};
use bon::builder as _;

#[state]
enum ProcessState {
    Init,
    NextState,
    OtherState,
    Finished,
}

#[machine]
struct ProcessMachine<ProcessState> {
    id: u64,
}

enum Decision {
    Next(ProcessMachine<NextState>),
    Other(ProcessMachine<OtherState>),
}

#[transition]
impl ProcessMachine<Init> {
    fn decide(self, event: u8) -> Decision {
        if event == 0 {
            Decision::Next(self.transition())
        } else {
            Decision::Other(self.transition())
        }
    }
}
