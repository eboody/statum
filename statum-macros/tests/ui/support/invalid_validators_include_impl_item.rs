struct DbRow {
    done: bool,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        if !self.done {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        if self.done {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}
