use super::*;
use statum_macros::{machine, state};

#[state]
pub enum WorkflowState {
    Draft,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    pub client: String,
}
