use super::*;
use statum_macros::validators;

pub struct Row {
    pub status: &'static str,
}

#[validators(super::flows::WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), Error> {
        let _ = &client;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }
}

pub fn assert_rebuild() {
    let rebuilt = super::flows::WorkflowMachine::rebuild(&Row { status: "draft" })
        .client("acme".to_owned())
        .build()
        .unwrap();

    match rebuilt {
        super::flows::workflow_machine::SomeState::Draft(machine) => {
            assert_eq!(machine.client, "acme");
        }
    }
}
