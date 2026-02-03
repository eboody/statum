use statum::{machine, state, validators}; use statum::Error;

#[state]
pub enum JobState {
    Queued,
    Running(JobData),
    Complete,
}

pub struct JobData {
    id: u64,
}

#[machine]
pub struct JobMachine<JobState> {
    worker: String,
}

pub struct JobRow {
    status: &'static str,
}

#[validators(JobMachine)]
impl JobRow {
    async fn is_queued(&self) -> Result<(), Error> {
        let _ = worker;
        if self.status == "queued" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    async fn is_running(&self) -> Result<JobData, Error> {
        let _ = worker;
        if self.status == "running" {
            Ok(JobData { id: 1 })
        } else {
            Err(Error::InvalidState)
        }
    }

    async fn is_complete(&self) -> Result<(), Error> {
        let _ = worker;
        if self.status == "complete" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }
}

fn main() {
    let row = JobRow { status: "queued" };
    let _ = row.machine_builder().worker("w1".to_string()).build();
}
