use clap::{Parser, Subcommand, ValueEnum};
use sqlx::{FromRow, SqlitePool, sqlite};
use statum::{machine, state, transition, validators};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

pub const DEFAULT_DB_PATH: &str = "target/statum-examples/deploy-pipeline.db";

const STATUS_DRAFT: &str = "draft";
const STATUS_PLANNED: &str = "planned";
const STATUS_AWAITING_APPROVAL: &str = "awaiting_approval";
const STATUS_APPLYING: &str = "applying";
const STATUS_APPLIED: &str = "applied";
const STATUS_ROLLED_BACK: &str = "rolled_back";
const STATUS_FAILED: &str = "failed";

#[state]
pub enum DeploymentState {
    Draft,
    Planned(PlanDigest),
    AwaitingApproval(ApprovalRequest),
    Applying(OperationId),
    Applied(ApplyReceipt),
    RolledBack(RollbackInfo),
    Failed(FailureInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanDigest {
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub ticket: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationId {
    pub operation_id: String,
    pub approved_by: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyReceipt {
    pub receipt_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollbackInfo {
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
    pub error: String,
}

#[machine]
pub struct DeploymentMachine<DeploymentState> {
    pub id: i64,
    pub service: String,
    pub environment: String,
    pub version: String,
}

#[transition]
impl DeploymentMachine<Draft> {
    fn plan(self) -> DeploymentMachine<Planned> {
        let digest = format!(
            "plan:{}:{}:{}",
            self.service, self.environment, self.version
        );
        self.transition_with(PlanDigest { digest })
    }
}

#[transition]
impl DeploymentMachine<Planned> {
    fn request_approval(self) -> DeploymentMachine<AwaitingApproval> {
        let ticket = format!("approval:{}:{}", self.id, self.version);
        self.transition_with(ApprovalRequest { ticket })
    }
}

#[transition]
impl DeploymentMachine<AwaitingApproval> {
    fn approve(self, approved_by: String) -> DeploymentMachine<Applying> {
        let operation_id = format!("op:{}:{}", self.id, self.version);
        self.transition_with(OperationId {
            operation_id,
            approved_by,
        })
    }
}

#[transition]
impl DeploymentMachine<Applying> {
    fn finish_success(self) -> DeploymentMachine<Applied> {
        let receipt_id = format!("receipt:{}:{}", self.id, self.version);
        self.transition_with(ApplyReceipt { receipt_id })
    }

    fn finish_failure(self, error: String) -> DeploymentMachine<Failed> {
        self.transition_with(FailureInfo { error })
    }
}

#[transition]
impl DeploymentMachine<Applied> {
    fn rollback(self, reason: String) -> DeploymentMachine<RolledBack> {
        self.transition_with(RollbackInfo { reason })
    }
}

#[derive(Clone, Debug, FromRow)]
struct DeploymentRow {
    id: i64,
    service: String,
    environment: String,
    version: String,
    status: String,
    plan_digest: Option<String>,
    approval_ticket: Option<String>,
    approved_by: Option<String>,
    operation_id: Option<String>,
    receipt_id: Option<String>,
    failure_reason: Option<String>,
    rollback_reason: Option<String>,
}

#[validators(DeploymentMachine)]
impl DeploymentRow {
    fn is_draft(&self) -> statum::Result<()> {
        if *id > 0
            && !service.is_empty()
            && !environment.is_empty()
            && !version.is_empty()
            && self.status == STATUS_DRAFT
            && self.plan_digest.is_none()
            && self.approval_ticket.is_none()
            && self.approved_by.is_none()
            && self.operation_id.is_none()
            && self.receipt_id.is_none()
            && self.failure_reason.is_none()
            && self.rollback_reason.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_planned(&self) -> statum::Result<PlanDigest> {
        if *id <= 0
            || service.is_empty()
            || environment.is_empty()
            || version.is_empty()
            || self.status != STATUS_PLANNED
            || self.approval_ticket.is_some()
            || self.approved_by.is_some()
            || self.operation_id.is_some()
            || self.receipt_id.is_some()
            || self.failure_reason.is_some()
            || self.rollback_reason.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.plan_digest
            .clone()
            .filter(|digest| !digest.trim().is_empty())
            .map(|digest| PlanDigest { digest })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_awaiting_approval(&self) -> statum::Result<ApprovalRequest> {
        if *id <= 0
            || service.is_empty()
            || environment.is_empty()
            || version.is_empty()
            || self.status != STATUS_AWAITING_APPROVAL
            || self.plan_digest.is_some()
            || self.approved_by.is_some()
            || self.operation_id.is_some()
            || self.receipt_id.is_some()
            || self.failure_reason.is_some()
            || self.rollback_reason.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.approval_ticket
            .clone()
            .filter(|ticket| !ticket.trim().is_empty())
            .map(|ticket| ApprovalRequest { ticket })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_applying(&self) -> statum::Result<OperationId> {
        if *id <= 0
            || service.is_empty()
            || environment.is_empty()
            || version.is_empty()
            || self.status != STATUS_APPLYING
            || self.plan_digest.is_some()
            || self.approval_ticket.is_some()
            || self.receipt_id.is_some()
            || self.failure_reason.is_some()
            || self.rollback_reason.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        match (&self.approved_by, &self.operation_id) {
            (Some(approved_by), Some(operation_id))
                if !approved_by.trim().is_empty() && !operation_id.trim().is_empty() =>
            {
                Ok(OperationId {
                    operation_id: operation_id.clone(),
                    approved_by: approved_by.clone(),
                })
            }
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_applied(&self) -> statum::Result<ApplyReceipt> {
        if *id <= 0
            || service.is_empty()
            || environment.is_empty()
            || version.is_empty()
            || self.status != STATUS_APPLIED
            || self.plan_digest.is_some()
            || self.approval_ticket.is_some()
            || self.approved_by.is_some()
            || self.operation_id.is_some()
            || self.failure_reason.is_some()
            || self.rollback_reason.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.receipt_id
            .clone()
            .filter(|receipt_id| !receipt_id.trim().is_empty())
            .map(|receipt_id| ApplyReceipt { receipt_id })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_rolled_back(&self) -> statum::Result<RollbackInfo> {
        if *id <= 0
            || service.is_empty()
            || environment.is_empty()
            || version.is_empty()
            || self.status != STATUS_ROLLED_BACK
            || self.plan_digest.is_some()
            || self.approval_ticket.is_some()
            || self.approved_by.is_some()
            || self.operation_id.is_some()
            || self.receipt_id.is_some()
            || self.failure_reason.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.rollback_reason
            .clone()
            .filter(|reason| !reason.trim().is_empty())
            .map(|reason| RollbackInfo { reason })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_failed(&self) -> statum::Result<FailureInfo> {
        if *id <= 0
            || service.is_empty()
            || environment.is_empty()
            || version.is_empty()
            || self.status != STATUS_FAILED
            || self.plan_digest.is_some()
            || self.approval_ticket.is_some()
            || self.approved_by.is_some()
            || self.operation_id.is_some()
            || self.receipt_id.is_some()
            || self.rollback_reason.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.failure_reason
            .clone()
            .filter(|error| !error.trim().is_empty())
            .map(|error| FailureInfo { error })
            .ok_or(statum::Error::InvalidState)
    }
}

#[derive(Parser)]
#[command(name = "clap-sqlite-deploy-pipeline")]
struct Cli {
    #[arg(long, global = true, default_value = DEFAULT_DB_PATH)]
    db: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Create {
        #[arg(long)]
        service: String,
        #[arg(long = "env")]
        environment: String,
        #[arg(long)]
        version: String,
    },
    Show {
        id: i64,
    },
    Plan {
        id: i64,
    },
    RequestApproval {
        id: i64,
    },
    Approve {
        id: i64,
        #[arg(long = "by")]
        approved_by: String,
    },
    FinishApply {
        id: i64,
        #[arg(long)]
        result: FinishApplyResult,
        #[arg(long)]
        reason: Option<String>,
    },
    Rollback {
        id: i64,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum FinishApplyResult {
    Success,
    Failure,
}

struct DeploymentStore {
    pool: SqlitePool,
}

struct PersistedPhase<'a> {
    status: &'static str,
    plan_digest: Option<&'a str>,
    approval_ticket: Option<&'a str>,
    approved_by: Option<&'a str>,
    operation_id: Option<&'a str>,
    receipt_id: Option<&'a str>,
    failure_reason: Option<&'a str>,
    rollback_reason: Option<&'a str>,
}

#[derive(Debug)]
pub enum CliError {
    Io(std::io::Error),
    CorruptState,
    Database(sqlx::Error),
    InvalidInput(&'static str),
    InvalidTransition(&'static str),
    NotFound(i64),
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::CorruptState => write!(f, "stored deployment row did not match any validator"),
            Self::Database(error) => write!(f, "{error}"),
            Self::InvalidInput(message) => write!(f, "{message}"),
            Self::InvalidTransition(message) => write!(f, "{message}"),
            Self::NotFound(id) => write!(f, "deployment {id} not found"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<sqlx::Error> for CliError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub async fn run_from_args<I, T>(args: I) -> Result<String, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    let store = DeploymentStore::open(&cli.db).await?;
    store.execute(cli.command).await
}

pub async fn run() -> Result<(), CliError> {
    let output = run_from_args(std::env::args_os()).await?;
    println!("{output}");
    Ok(())
}

impl DeploymentStore {
    async fn open(db_path: &Path) -> Result<Self, CliError> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let options = sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);
        let pool = sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let store = Self { pool };
        store.init_schema().await?;
        Ok(store)
    }

    async fn execute(&self, command: Command) -> Result<String, CliError> {
        match command {
            Command::Create {
                service,
                environment,
                version,
            } => {
                ensure_non_empty(&service, "service is required")?;
                ensure_non_empty(&environment, "environment is required")?;
                ensure_non_empty(&version, "version is required")?;

                let id = self.insert_draft(&service, &environment, &version).await?;
                self.show(id).await
            }
            Command::Show { id } => self.show(id).await,
            Command::Plan { id } => {
                let state = self.load_state(id).await?;
                let machine = match state {
                    deployment_machine::SomeState::Draft(machine) => machine.plan(),
                    _ => {
                        return Err(CliError::InvalidTransition(
                            "plan requires a draft deployment",
                        ));
                    }
                };
                self.persist_planned(&machine).await?;
                self.show(id).await
            }
            Command::RequestApproval { id } => {
                let state = self.load_state(id).await?;
                let machine = match state {
                    deployment_machine::SomeState::Planned(machine) => machine.request_approval(),
                    _ => {
                        return Err(CliError::InvalidTransition(
                            "request-approval requires a planned deployment",
                        ));
                    }
                };
                self.persist_awaiting_approval(&machine).await?;
                self.show(id).await
            }
            Command::Approve { id, approved_by } => {
                ensure_non_empty(&approved_by, "approved_by is required")?;
                let state = self.load_state(id).await?;
                let machine = match state {
                    deployment_machine::SomeState::AwaitingApproval(machine) => {
                        machine.approve(approved_by)
                    }
                    _ => {
                        return Err(CliError::InvalidTransition(
                            "approve requires an awaiting-approval deployment",
                        ));
                    }
                };
                self.persist_applying(&machine).await?;
                self.show(id).await
            }
            Command::FinishApply { id, result, reason } => {
                let state = self.load_state(id).await?;
                let machine = match state {
                    deployment_machine::SomeState::Applying(machine) => machine,
                    _ => {
                        return Err(CliError::InvalidTransition(
                            "finish-apply requires an applying deployment",
                        ));
                    }
                };

                match result {
                    FinishApplyResult::Success => {
                        if reason.is_some() {
                            return Err(CliError::InvalidInput(
                                "reason is only valid when --result failure",
                            ));
                        }
                        let machine = machine.finish_success();
                        self.persist_applied(&machine).await?;
                    }
                    FinishApplyResult::Failure => {
                        let reason = reason.ok_or(CliError::InvalidInput(
                            "reason is required when --result failure",
                        ))?;
                        ensure_non_empty(&reason, "reason is required when --result failure")?;
                        let machine = machine.finish_failure(reason);
                        self.persist_failed(&machine).await?;
                    }
                }

                self.show(id).await
            }
            Command::Rollback { id, reason } => {
                ensure_non_empty(&reason, "reason is required")?;
                let state = self.load_state(id).await?;
                let machine = match state {
                    deployment_machine::SomeState::Applied(machine) => machine.rollback(reason),
                    _ => {
                        return Err(CliError::InvalidTransition(
                            "rollback requires an applied deployment",
                        ));
                    }
                };
                self.persist_rolled_back(&machine).await?;
                self.show(id).await
            }
        }
    }

    async fn init_schema(&self) -> Result<(), CliError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS deployments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service TEXT NOT NULL,
                environment TEXT NOT NULL,
                version TEXT NOT NULL,
                status TEXT NOT NULL,
                plan_digest TEXT,
                approval_ticket TEXT,
                approved_by TEXT,
                operation_id TEXT,
                receipt_id TEXT,
                failure_reason TEXT,
                rollback_reason TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn insert_draft(
        &self,
        service: &str,
        environment: &str,
        version: &str,
    ) -> Result<i64, CliError> {
        let result = sqlx::query(
            r#"
            INSERT INTO deployments (
                service,
                environment,
                version,
                status,
                plan_digest,
                approval_ticket,
                approved_by,
                operation_id,
                receipt_id,
                failure_reason,
                rollback_reason
            )
            VALUES (?, ?, ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL)
            "#,
        )
        .bind(service)
        .bind(environment)
        .bind(version)
        .bind(STATUS_DRAFT)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn load_state(&self, id: i64) -> Result<deployment_machine::SomeState, CliError> {
        let row = self.fetch_row(id).await?;
        row.clone()
            .into_machine()
            .id(row.id)
            .service(row.service)
            .environment(row.environment)
            .version(row.version)
            .build()
            .map_err(|_| CliError::CorruptState)
    }

    async fn show(&self, id: i64) -> Result<String, CliError> {
        let state = self.load_state(id).await?;
        Ok(format_summary(state))
    }

    async fn fetch_row(&self, id: i64) -> Result<DeploymentRow, CliError> {
        let row = sqlx::query_as::<_, DeploymentRow>(
            r#"
            SELECT
                id,
                service,
                environment,
                version,
                status,
                plan_digest,
                approval_ticket,
                approved_by,
                operation_id,
                receipt_id,
                failure_reason,
                rollback_reason
            FROM deployments
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.ok_or(CliError::NotFound(id))
    }

    async fn persist_state(&self, id: i64, phase: PersistedPhase<'_>) -> Result<(), CliError> {
        sqlx::query(
            r#"
            UPDATE deployments
            SET
                status = ?,
                plan_digest = ?,
                approval_ticket = ?,
                approved_by = ?,
                operation_id = ?,
                receipt_id = ?,
                failure_reason = ?,
                rollback_reason = ?
            WHERE id = ?
            "#,
        )
        .bind(phase.status)
        .bind(phase.plan_digest)
        .bind(phase.approval_ticket)
        .bind(phase.approved_by)
        .bind(phase.operation_id)
        .bind(phase.receipt_id)
        .bind(phase.failure_reason)
        .bind(phase.rollback_reason)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn persist_planned(&self, machine: &DeploymentMachine<Planned>) -> Result<(), CliError> {
        self.persist_state(
            machine.id,
            PersistedPhase {
                status: STATUS_PLANNED,
                plan_digest: Some(&machine.state_data.digest),
                approval_ticket: None,
                approved_by: None,
                operation_id: None,
                receipt_id: None,
                failure_reason: None,
                rollback_reason: None,
            },
        )
        .await
    }

    async fn persist_awaiting_approval(
        &self,
        machine: &DeploymentMachine<AwaitingApproval>,
    ) -> Result<(), CliError> {
        self.persist_state(
            machine.id,
            PersistedPhase {
                status: STATUS_AWAITING_APPROVAL,
                plan_digest: None,
                approval_ticket: Some(&machine.state_data.ticket),
                approved_by: None,
                operation_id: None,
                receipt_id: None,
                failure_reason: None,
                rollback_reason: None,
            },
        )
        .await
    }

    async fn persist_applying(
        &self,
        machine: &DeploymentMachine<Applying>,
    ) -> Result<(), CliError> {
        self.persist_state(
            machine.id,
            PersistedPhase {
                status: STATUS_APPLYING,
                plan_digest: None,
                approval_ticket: None,
                approved_by: Some(&machine.state_data.approved_by),
                operation_id: Some(&machine.state_data.operation_id),
                receipt_id: None,
                failure_reason: None,
                rollback_reason: None,
            },
        )
        .await
    }

    async fn persist_applied(&self, machine: &DeploymentMachine<Applied>) -> Result<(), CliError> {
        self.persist_state(
            machine.id,
            PersistedPhase {
                status: STATUS_APPLIED,
                plan_digest: None,
                approval_ticket: None,
                approved_by: None,
                operation_id: None,
                receipt_id: Some(&machine.state_data.receipt_id),
                failure_reason: None,
                rollback_reason: None,
            },
        )
        .await
    }

    async fn persist_failed(&self, machine: &DeploymentMachine<Failed>) -> Result<(), CliError> {
        self.persist_state(
            machine.id,
            PersistedPhase {
                status: STATUS_FAILED,
                plan_digest: None,
                approval_ticket: None,
                approved_by: None,
                operation_id: None,
                receipt_id: None,
                failure_reason: Some(&machine.state_data.error),
                rollback_reason: None,
            },
        )
        .await
    }

    async fn persist_rolled_back(
        &self,
        machine: &DeploymentMachine<RolledBack>,
    ) -> Result<(), CliError> {
        self.persist_state(
            machine.id,
            PersistedPhase {
                status: STATUS_ROLLED_BACK,
                plan_digest: None,
                approval_ticket: None,
                approved_by: None,
                operation_id: None,
                receipt_id: None,
                failure_reason: None,
                rollback_reason: Some(&machine.state_data.reason),
            },
        )
        .await
    }
}

fn ensure_non_empty(value: &str, message: &'static str) -> Result<(), CliError> {
    if value.trim().is_empty() {
        Err(CliError::InvalidInput(message))
    } else {
        Ok(())
    }
}

fn common_summary<T: DeploymentStateTrait>(machine: &DeploymentMachine<T>) -> Vec<String> {
    vec![
        format!("id={}", machine.id),
        format!("service={}", machine.service),
        format!("environment={}", machine.environment),
        format!("version={}", machine.version),
    ]
}

fn format_summary(state: deployment_machine::SomeState) -> String {
    let mut lines = match state {
        deployment_machine::SomeState::Draft(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=draft".to_string());
            lines
        }
        deployment_machine::SomeState::Planned(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=planned".to_string());
            lines.push(format!("plan_digest={}", machine.state_data.digest));
            lines
        }
        deployment_machine::SomeState::AwaitingApproval(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=awaiting_approval".to_string());
            lines.push(format!("approval_ticket={}", machine.state_data.ticket));
            lines
        }
        deployment_machine::SomeState::Applying(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=applying".to_string());
            lines.push(format!("operation_id={}", machine.state_data.operation_id));
            lines.push(format!("approved_by={}", machine.state_data.approved_by));
            lines
        }
        deployment_machine::SomeState::Applied(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=applied".to_string());
            lines.push(format!("receipt_id={}", machine.state_data.receipt_id));
            lines
        }
        deployment_machine::SomeState::RolledBack(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=rolled_back".to_string());
            lines.push(format!("rollback_reason={}", machine.state_data.reason));
            lines
        }
        deployment_machine::SomeState::Failed(machine) => {
            let mut lines = common_summary(&machine);
            lines.push("state=failed".to_string());
            lines.push(format!("error={}", machine.state_data.error));
            lines
        }
    };

    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn row_rehydrates_into_applying_state() {
        let tmp_dir = std::env::temp_dir().join("statum-showcases");
        let db_path = tmp_dir.join("deploy-pipeline-unit.db");
        let _ = fs::remove_file(&db_path);

        let store = DeploymentStore::open(&db_path).await.unwrap();
        let created = store
            .execute(Command::Create {
                service: "api".to_string(),
                environment: "prod".to_string(),
                version: "1.2.3".to_string(),
            })
            .await
            .unwrap();
        let id = created
            .lines()
            .find_map(|line| line.strip_prefix("id="))
            .unwrap()
            .parse::<i64>()
            .unwrap();

        store.execute(Command::Plan { id }).await.unwrap();
        store
            .execute(Command::RequestApproval { id })
            .await
            .unwrap();
        store
            .execute(Command::Approve {
                id,
                approved_by: "alice".to_string(),
            })
            .await
            .unwrap();

        let state = store.load_state(id).await.unwrap();
        match state {
            deployment_machine::SomeState::Applying(machine) => {
                assert_eq!(machine.state_data.operation_id.as_str(), "op:1:1.2.3");
                assert_eq!(machine.state_data.approved_by.as_str(), "alice");
            }
            _ => panic!("expected an applying deployment"),
        }

        let _ = fs::remove_file(&db_path);
    }
}
