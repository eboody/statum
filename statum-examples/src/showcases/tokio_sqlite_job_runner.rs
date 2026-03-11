use sqlx::{FromRow, SqlitePool, sqlite::SqlitePoolOptions};
use statum::{machine, state, transition, validators};

pub const RETRY_DELAY_MS: i64 = 1_000;

const STATUS_QUEUED: &str = "queued";
const STATUS_RUNNING: &str = "running";
const STATUS_RETRYING: &str = "retrying";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

#[state]
pub enum JobState {
    Queued,
    Running(JobLease),
    Retrying(RetryPlan),
    Succeeded(JobResult),
    Failed(FailureInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobLease {
    pub lease_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPlan {
    pub available_at_ms: i64,
    pub last_error: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobResult {
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
    pub error: String,
}

#[machine]
pub struct JobMachine<JobState> {
    pub id: i64,
    pub job_type: String,
    pub payload: String,
    pub attempts: i64,
    pub max_attempts: i64,
}

#[transition]
impl JobMachine<Queued> {
    fn start(mut self, lease_token: String) -> JobMachine<Running> {
        self.attempts += 1;
        self.transition_with(JobLease { lease_token })
    }
}

#[transition]
impl JobMachine<Retrying> {
    fn restart(mut self, lease_token: String) -> JobMachine<Running> {
        self.attempts += 1;
        self.transition_with(JobLease { lease_token })
    }
}

#[transition]
impl JobMachine<Running> {
    fn succeed(self, output: String) -> JobMachine<Succeeded> {
        self.transition_with(JobResult { output })
    }

    fn retry(self, available_at_ms: i64, error: String) -> JobMachine<Retrying> {
        self.transition_with(RetryPlan {
            available_at_ms,
            last_error: error,
        })
    }

    fn fail(self, error: String) -> JobMachine<Failed> {
        self.transition_with(FailureInfo { error })
    }
}

#[derive(Clone, Debug, FromRow)]
struct JobRow {
    id: i64,
    job_type: String,
    payload: String,
    attempts: i64,
    max_attempts: i64,
    status: String,
    available_at_ms: i64,
    lease_token: Option<String>,
    last_error: Option<String>,
    result_text: Option<String>,
}

#[validators(JobMachine)]
impl JobRow {
    fn is_queued(&self) -> statum::Result<()> {
        if *id > 0
            && !job_type.is_empty()
            && !payload.is_empty()
            && *attempts == 0
            && *max_attempts > 0
            && self.status == STATUS_QUEUED
            && self.available_at_ms == 0
            && self.lease_token.is_none()
            && self.last_error.is_none()
            && self.result_text.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_running(&self) -> statum::Result<JobLease> {
        if *id <= 0
            || job_type.is_empty()
            || payload.is_empty()
            || *attempts <= 0
            || *attempts > *max_attempts
            || self.status != STATUS_RUNNING
            || self.available_at_ms != 0
            || self.last_error.is_some()
            || self.result_text.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.lease_token
            .clone()
            .map(|lease_token| JobLease { lease_token })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_retrying(&self) -> statum::Result<RetryPlan> {
        if *id <= 0
            || job_type.is_empty()
            || payload.is_empty()
            || *attempts <= 0
            || *attempts >= *max_attempts
            || self.status != STATUS_RETRYING
            || self.available_at_ms <= 0
            || self.result_text.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.last_error
            .clone()
            .filter(|last_error| !last_error.trim().is_empty())
            .map(|last_error| RetryPlan {
                available_at_ms: self.available_at_ms,
                last_error,
            })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_succeeded(&self) -> statum::Result<JobResult> {
        if *id <= 0
            || job_type.is_empty()
            || payload.is_empty()
            || *attempts <= 0
            || *attempts > *max_attempts
            || self.status != STATUS_SUCCEEDED
            || self.available_at_ms != 0
            || self.lease_token.is_some()
            || self.last_error.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.result_text
            .clone()
            .map(|output| JobResult { output })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_failed(&self) -> statum::Result<FailureInfo> {
        if *id <= 0
            || job_type.is_empty()
            || payload.is_empty()
            || *attempts <= 0
            || *attempts != *max_attempts
            || self.status != STATUS_FAILED
            || self.available_at_ms != 0
            || self.lease_token.is_some()
            || self.result_text.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.last_error
            .clone()
            .map(|error| FailureInfo { error })
            .ok_or(statum::Error::InvalidState)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSnapshot {
    pub id: i64,
    pub job_type: String,
    pub payload: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub status: String,
    pub available_at_ms: i64,
    pub lease_token: Option<String>,
    pub last_error: Option<String>,
    pub result_text: Option<String>,
}

impl From<JobRow> for JobSnapshot {
    fn from(row: JobRow) -> Self {
        Self {
            id: row.id,
            job_type: row.job_type,
            payload: row.payload,
            attempts: row.attempts,
            max_attempts: row.max_attempts,
            status: row.status,
            available_at_ms: row.available_at_ms,
            lease_token: row.lease_token,
            last_error: row.last_error,
            result_text: row.result_text,
        }
    }
}

#[derive(Debug)]
pub enum RunnerError {
    CorruptState,
    Database(sqlx::Error),
    InvalidJobConfig(&'static str),
    UnexpectedRunnableState,
}

impl core::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CorruptState => write!(f, "stored job row did not match any validator"),
            Self::Database(error) => write!(f, "{error}"),
            Self::InvalidJobConfig(message) => write!(f, "{message}"),
            Self::UnexpectedRunnableState => write!(f, "runner selected a non-runnable state"),
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<sqlx::Error> for RunnerError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub struct JobRunner {
    pool: SqlitePool,
    now_ms: i64,
    lease_sequence: u64,
}

enum ProcessedJob {
    Retrying(JobMachine<Retrying>),
    Succeeded(JobMachine<Succeeded>),
    Failed(JobMachine<Failed>),
}

pub async fn build_runner() -> Result<JobRunner, RunnerError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            attempts INTEGER NOT NULL,
            max_attempts INTEGER NOT NULL,
            status TEXT NOT NULL,
            available_at_ms INTEGER NOT NULL,
            lease_token TEXT,
            last_error TEXT,
            result_text TEXT
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(JobRunner {
        pool,
        now_ms: 0,
        lease_sequence: 0,
    })
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut runner = build_runner().await?;
    runner.enqueue("email", "ok", 3).await?;
    runner.enqueue("thumbnail", "flaky", 3).await?;
    runner.enqueue("invoice", "always-fail", 2).await?;

    runner.run_until_idle().await?;

    for job in runner.jobs().await? {
        println!(
            "job={} type={} status={} attempts={}/{} result={:?} error={:?}",
            job.id,
            job.job_type,
            job.status,
            job.attempts,
            job.max_attempts,
            job.result_text,
            job.last_error,
        );
    }

    Ok(())
}

impl JobRunner {
    pub async fn enqueue(
        &self,
        job_type: &str,
        payload: &str,
        max_attempts: i64,
    ) -> Result<i64, RunnerError> {
        if job_type.trim().is_empty() {
            return Err(RunnerError::InvalidJobConfig("job_type is required"));
        }
        if payload.trim().is_empty() {
            return Err(RunnerError::InvalidJobConfig("payload is required"));
        }
        if max_attempts <= 0 {
            return Err(RunnerError::InvalidJobConfig(
                "max_attempts must be greater than zero",
            ));
        }

        let result = sqlx::query(
            r#"
            INSERT INTO jobs (
                job_type,
                payload,
                attempts,
                max_attempts,
                status,
                available_at_ms,
                lease_token,
                last_error,
                result_text
            )
            VALUES (?, ?, 0, ?, ?, 0, NULL, NULL, NULL)
            "#,
        )
        .bind(job_type)
        .bind(payload)
        .bind(max_attempts)
        .bind(STATUS_QUEUED)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub fn advance_time(&mut self, delta_ms: i64) {
        self.now_ms += delta_ms;
    }

    pub async fn run_next(&mut self) -> Result<bool, RunnerError> {
        let Some(row) = self.next_runnable_row().await? else {
            return Ok(false);
        };

        let machine = self.rehydrate_row(row.clone())?;
        let running = match machine {
            job_machine::State::Queued(machine) => machine.start(self.next_lease_token()),
            job_machine::State::Retrying(machine) => machine.restart(self.next_lease_token()),
            _ => return Err(RunnerError::UnexpectedRunnableState),
        };

        self.persist_running(&running).await?;

        match self.process_running_job(running).await {
            ProcessedJob::Retrying(machine) => self.persist_retrying(&machine).await?,
            ProcessedJob::Succeeded(machine) => self.persist_succeeded(&machine).await?,
            ProcessedJob::Failed(machine) => self.persist_failed(&machine).await?,
        }

        Ok(true)
    }

    pub async fn run_until_idle(&mut self) -> Result<(), RunnerError> {
        loop {
            if self.run_next().await? {
                continue;
            }

            let Some(next_available_at) = self.next_retry_at().await? else {
                break;
            };

            if next_available_at > self.now_ms {
                self.now_ms = next_available_at;
                continue;
            }

            break;
        }

        Ok(())
    }

    pub async fn snapshot(&self, id: i64) -> Result<JobSnapshot, RunnerError> {
        let row = self.fetch_row(id).await?;
        Ok(row.into())
    }

    pub async fn jobs(&self) -> Result<Vec<JobSnapshot>, RunnerError> {
        let rows = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT
                id,
                job_type,
                payload,
                attempts,
                max_attempts,
                status,
                available_at_ms,
                lease_token,
                last_error,
                result_text
            FROM jobs
            ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(JobSnapshot::from).collect())
    }

    async fn next_runnable_row(&self) -> Result<Option<JobRow>, RunnerError> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT
                id,
                job_type,
                payload,
                attempts,
                max_attempts,
                status,
                available_at_ms,
                lease_token,
                last_error,
                result_text
            FROM jobs
            WHERE status = ?
               OR (status = ? AND available_at_ms <= ?)
            ORDER BY available_at_ms, id
            LIMIT 1
            "#,
        )
        .bind(STATUS_QUEUED)
        .bind(STATUS_RETRYING)
        .bind(self.now_ms)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn next_retry_at(&self) -> Result<Option<i64>, RunnerError> {
        let next = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT MIN(available_at_ms)
            FROM jobs
            WHERE status = ?
            "#,
        )
        .bind(STATUS_RETRYING)
        .fetch_optional(&self.pool)
        .await?;

        Ok(next)
    }

    async fn fetch_row(&self, id: i64) -> Result<JobRow, RunnerError> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT
                id,
                job_type,
                payload,
                attempts,
                max_attempts,
                status,
                available_at_ms,
                lease_token,
                last_error,
                result_text
            FROM jobs
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    fn rehydrate_row(&self, row: JobRow) -> Result<job_machine::State, RunnerError> {
        row.clone()
            .into_machine()
            .id(row.id)
            .job_type(row.job_type)
            .payload(row.payload)
            .attempts(row.attempts)
            .max_attempts(row.max_attempts)
            .build()
            .map_err(|_| RunnerError::CorruptState)
    }

    fn next_lease_token(&mut self) -> String {
        self.lease_sequence += 1;
        format!("lease-{}-{}", self.now_ms, self.lease_sequence)
    }

    async fn process_running_job(&mut self, machine: JobMachine<Running>) -> ProcessedJob {
        tokio::task::yield_now().await;

        let payload = machine.payload.clone();
        let job_type = machine.job_type.clone();
        let attempts = machine.attempts;
        let max_attempts = machine.max_attempts;

        match payload.as_str() {
            "ok" => ProcessedJob::Succeeded(
                machine.succeed(format!("{job_type} completed on attempt {attempts}")),
            ),
            "flaky" if attempts == 1 => ProcessedJob::Retrying(machine.retry(
                self.now_ms + RETRY_DELAY_MS,
                "temporary upstream timeout".to_string(),
            )),
            "flaky" => ProcessedJob::Succeeded(
                machine.succeed(format!("{job_type} recovered on attempt {attempts}")),
            ),
            "always-fail" if attempts < max_attempts => ProcessedJob::Retrying(machine.retry(
                self.now_ms + RETRY_DELAY_MS,
                format!("attempt {attempts} failed permanently later"),
            )),
            "always-fail" => ProcessedJob::Failed(machine.fail(format!(
                "{job_type} exhausted after {max_attempts} attempts"
            ))),
            other => ProcessedJob::Failed(
                machine.fail(format!("{job_type} does not understand payload `{other}`")),
            ),
        }
    }

    async fn persist_running(&self, machine: &JobMachine<Running>) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET attempts = ?, status = ?, available_at_ms = 0, lease_token = ?, last_error = NULL, result_text = NULL
            WHERE id = ?
            "#,
        )
        .bind(machine.attempts)
        .bind(STATUS_RUNNING)
        .bind(&machine.state_data.lease_token)
        .bind(machine.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn persist_retrying(&self, machine: &JobMachine<Retrying>) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET attempts = ?, status = ?, available_at_ms = ?, lease_token = NULL, last_error = ?, result_text = NULL
            WHERE id = ?
            "#,
        )
        .bind(machine.attempts)
        .bind(STATUS_RETRYING)
        .bind(machine.state_data.available_at_ms)
        .bind(&machine.state_data.last_error)
        .bind(machine.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn persist_succeeded(&self, machine: &JobMachine<Succeeded>) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET attempts = ?, status = ?, available_at_ms = 0, lease_token = NULL, last_error = NULL, result_text = ?
            WHERE id = ?
            "#,
        )
        .bind(machine.attempts)
        .bind(STATUS_SUCCEEDED)
        .bind(&machine.state_data.output)
        .bind(machine.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn persist_failed(&self, machine: &JobMachine<Failed>) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET attempts = ?, status = ?, available_at_ms = 0, lease_token = NULL, last_error = ?, result_text = NULL
            WHERE id = ?
            "#,
        )
        .bind(machine.attempts)
        .bind(STATUS_FAILED)
        .bind(&machine.state_data.error)
        .bind(machine.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn row_rehydrates_into_retrying_machine_state() {
        let mut runner = build_runner().await.unwrap();
        let job_id = runner.enqueue("thumbnail", "flaky", 3).await.unwrap();

        assert!(runner.run_next().await.unwrap());

        let row = runner.fetch_row(job_id).await.unwrap();
        let state = runner.rehydrate_row(row).unwrap();

        match state {
            job_machine::State::Retrying(machine) => {
                assert_eq!(machine.attempts, 1);
                assert_eq!(machine.state_data.available_at_ms, RETRY_DELAY_MS);
                assert_eq!(
                    machine.state_data.last_error.as_str(),
                    "temporary upstream timeout"
                );
            }
            _ => panic!("expected a retrying machine"),
        }
    }
}
