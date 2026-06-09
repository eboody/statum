use axum::{
    Json, Router,
    extract::{Path, State},
    http, response as reply,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, sqlite};
use statum::{
    MachineIntrospection, MachineTransitionRecorder, StableGraphMetadata,
    TransitionTelemetryLabels, machine, state, transition, validators,
};

const STATUS_DRAFT: &str = "draft";
const STATUS_IN_REVIEW: &str = "in_review";
const STATUS_PUBLISHED: &str = "published";

#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
    Published,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReviewAssignment {
    reviewer: String,
}

#[machine]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

#[transition]
impl DocumentMachine<Draft> {
    fn submit(self, reviewer: String) -> DocumentMachine<InReview> {
        self.transition_with(ReviewAssignment { reviewer })
    }
}

#[transition]
impl DocumentMachine<InReview> {
    fn approve(self) -> DocumentMachine<Published> {
        self.transition()
    }
}

#[derive(Clone, Debug, FromRow)]
struct DocumentRow {
    id: i64,
    title: String,
    body: String,
    status: String,
    reviewer: Option<String>,
}

impl From<DocumentRow> for DocumentResponse {
    fn from(row: DocumentRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            body: row.body,
            status: row.status,
            reviewer: row.reviewer,
        }
    }
}

#[validators(DocumentMachine)]
impl DocumentRow {
    fn is_draft(&self) -> statum::Result<()> {
        if *id > 0
            && !title.is_empty()
            && !body.is_empty()
            && self.status == STATUS_DRAFT
            && self.reviewer.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_review(&self) -> statum::Result<ReviewAssignment> {
        if *id <= 0 || title.is_empty() || body.is_empty() || self.status != STATUS_IN_REVIEW {
            return Err(statum::Error::InvalidState);
        }

        self.reviewer
            .clone()
            .filter(|reviewer| !reviewer.trim().is_empty())
            .map(|reviewer| ReviewAssignment { reviewer })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_published(&self) -> statum::Result<()> {
        if *id > 0
            && !title.is_empty()
            && !body.is_empty()
            && self.status == STATUS_PUBLISHED
            && self.reviewer.is_none()
        {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
}

#[derive(Debug, Deserialize)]
struct CreateDocumentRequest {
    title: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct SubmitDocumentRequest {
    reviewer: String,
}

#[derive(Debug, Serialize)]
struct DocumentResponse {
    id: i64,
    title: String,
    body: String,
    status: String,
    reviewer: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Debug)]
enum AppError {
    BadRequest(&'static str),
    NotFound,
    InvalidTransition(&'static str),
    CorruptState,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl reply::IntoResponse for AppError {
    fn into_response(self) -> reply::Response {
        let (status, error) = match self {
            Self::BadRequest(message) => (http::StatusCode::BAD_REQUEST, message),
            Self::NotFound => (http::StatusCode::NOT_FOUND, "document not found"),
            Self::InvalidTransition(message) => (http::StatusCode::CONFLICT, message),
            Self::CorruptState => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "stored document row did not match any validator",
            ),
            Self::Database(_) => (http::StatusCode::INTERNAL_SERVER_ERROR, "database error"),
        };

        (status, Json(ErrorResponse { error })).into_response()
    }
}

pub async fn build_app() -> Result<Router, sqlx::Error> {
    let pool = build_pool().await?;
    init_schema(&pool).await?;
    Ok(router(pool))
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = build_app().await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/documents", post(create_document))
        .route("/documents/{id}", get(get_document))
        .route("/documents/{id}/submit", post(submit_document))
        .route("/documents/{id}/approve", post(approve_document))
        .with_state(AppState { pool })
}

pub fn workflow_graph_edges() -> Vec<String> {
    let graph = <DocumentMachine<Draft> as MachineIntrospection>::GRAPH;

    let mut edges = graph
        .transitions
        .iter()
        .flat_map(|transition| {
            let from = graph
                .state(transition.from)
                .map(|state| state.rust_name)
                .unwrap_or("<unknown>");

            transition.to.iter().map(move |target| {
                let to = graph
                    .state(*target)
                    .map(|state| state.rust_name)
                    .unwrap_or("<unknown>");

                format!("{from} --{}--> {to}", transition.method_name)
            })
        })
        .collect::<Vec<_>>();
    edges.sort();
    edges
}

pub fn workflow_stable_graph_metadata() -> StableGraphMetadata {
    let graph = <DocumentMachine<Draft> as MachineIntrospection>::GRAPH;
    StableGraphMetadata::from_graph(graph)
}

pub fn workflow_mermaid_diagram() -> String {
    workflow_stable_graph_metadata().to_mermaid_state_diagram()
}

pub fn workflow_dot_graph() -> String {
    workflow_stable_graph_metadata().to_dot_graph()
}

/// Demonstrates recording transition span labels without depending on a telemetry crate.
///
/// Applications can pass these low-cardinality labels to `tracing`, OpenTelemetry,
/// metrics, logs, or another stack. Statum only returns stable strings derived
/// from generated machine metadata.
pub fn example_transition_span_labels() -> Vec<String> {
    let graph = <DocumentMachine<Draft> as MachineIntrospection>::GRAPH;
    let events = [
        <DocumentMachine<Draft> as MachineTransitionRecorder>::try_record_transition_to::<
            DocumentMachine<InReview>,
        >(DocumentMachine::<Draft>::SUBMIT)
        .expect("submit transition should be statically known"),
        <DocumentMachine<InReview> as MachineTransitionRecorder>::try_record_transition_to::<
            DocumentMachine<Published>,
        >(DocumentMachine::<InReview>::APPROVE)
        .expect("approve transition should be statically known"),
    ];

    events
        .iter()
        .filter_map(|event| event.telemetry_labels_in(graph))
        .map(render_span_record)
        .collect()
}

fn render_span_record(labels: TransitionTelemetryLabels) -> String {
    format!(
        "span statum.transition machine={} from={} transition={} chosen={}",
        labels.machine, labels.from_state, labels.transition, labels.chosen_state
    )
}

async fn build_pool() -> Result<SqlitePool, sqlx::Error> {
    sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
}

async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            reviewer TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_document(
    State(app): State<AppState>,
    Json(request): Json<CreateDocumentRequest>,
) -> Result<(http::StatusCode, Json<DocumentResponse>), AppError> {
    if request.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required"));
    }
    if request.body.trim().is_empty() {
        return Err(AppError::BadRequest("body is required"));
    }

    let result = sqlx::query(
        r#"
        INSERT INTO documents (title, body, status, reviewer)
        VALUES (?, ?, ?, NULL)
        "#,
    )
    .bind(request.title)
    .bind(request.body)
    .bind(STATUS_DRAFT)
    .execute(&app.pool)
    .await?;

    let row = fetch_document_row(&app.pool, result.last_insert_rowid()).await?;
    Ok((http::StatusCode::CREATED, Json(row.into())))
}

async fn get_document(
    State(app): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DocumentResponse>, AppError> {
    let row = fetch_document_row(&app.pool, id).await?;
    Ok(Json(row.into()))
}

async fn submit_document(
    State(app): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<SubmitDocumentRequest>,
) -> Result<Json<DocumentResponse>, AppError> {
    if request.reviewer.trim().is_empty() {
        return Err(AppError::BadRequest("reviewer is required"));
    }

    let machine = load_document_state(&app.pool, id).await?;

    let machine = match machine {
        document_machine::SomeState::Draft(machine) => machine.submit(request.reviewer),
        _ => {
            return Err(AppError::InvalidTransition(
                "submit requires a draft document",
            ));
        }
    };

    persist_in_review(&app.pool, &machine).await?;

    let row = fetch_document_row(&app.pool, id).await?;
    Ok(Json(row.into()))
}

async fn approve_document(
    State(app): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DocumentResponse>, AppError> {
    let machine = load_document_state(&app.pool, id).await?;

    let machine = match machine {
        document_machine::SomeState::InReview(machine) => machine.approve(),
        _ => {
            return Err(AppError::InvalidTransition(
                "approve requires an in-review document",
            ));
        }
    };

    persist_published(&app.pool, &machine).await?;

    let row = fetch_document_row(&app.pool, id).await?;
    Ok(Json(row.into()))
}

async fn fetch_document_row(pool: &SqlitePool, id: i64) -> Result<DocumentRow, AppError> {
    let row = sqlx::query_as::<_, DocumentRow>(
        r#"
        SELECT id, title, body, status, reviewer
        FROM documents
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    row.ok_or(AppError::NotFound)
}

async fn load_document_state(
    pool: &SqlitePool,
    id: i64,
) -> Result<document_machine::SomeState, AppError> {
    let row = fetch_document_row(pool, id).await?;
    rebuild_document_row(&row)
        .into_result()
        .map_err(|_| AppError::CorruptState)
}

fn rebuild_document_row(row: &DocumentRow) -> statum::RebuildReport<document_machine::SomeState> {
    DocumentMachine::rebuild(row)
        .id(row.id)
        .title(row.title.clone())
        .body(row.body.clone())
        .build_report()
}

async fn persist_in_review(
    pool: &SqlitePool,
    machine: &DocumentMachine<InReview>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE documents
        SET title = ?, body = ?, status = ?, reviewer = ?
        WHERE id = ?
        "#,
    )
    .bind(&machine.title)
    .bind(&machine.body)
    .bind(STATUS_IN_REVIEW)
    .bind(&machine.state_data.reviewer)
    .bind(machine.id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn persist_published(
    pool: &SqlitePool,
    machine: &DocumentMachine<Published>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE documents
        SET title = ?, body = ?, status = ?, reviewer = NULL
        WHERE id = ?
        "#,
    )
    .bind(&machine.title)
    .bind(&machine.body)
    .bind(STATUS_PUBLISHED)
    .bind(machine.id)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use statum::testing::graph;

    #[test]
    fn canonical_graph_assertions_cover_allowed_and_forbidden_transitions() {
        let graph = <DocumentMachine<Draft> as MachineIntrospection>::GRAPH;

        graph::assert_transition(graph)
            .from(document_machine::StateId::Draft)
            .method("submit")
            .to(document_machine::StateId::InReview);
        graph::assert_transition(graph)
            .from(document_machine::StateId::InReview)
            .method("approve")
            .to(document_machine::StateId::Published);
        graph::assert_targets(graph)
            .from(document_machine::StateId::Draft)
            .method("submit")
            .exactly([document_machine::StateId::InReview]);
        graph::assert_no_transition(graph)
            .from(document_machine::StateId::Draft)
            .method("approve");
    }

    #[tokio::test]
    async fn persisted_row_rebuilds_into_state_discriminated_result() {
        let pool = build_pool().await.unwrap();
        init_schema(&pool).await.unwrap();

        let result = sqlx::query(
            r#"
            INSERT INTO documents (title, body, status, reviewer)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind("RFC")
        .bind("ready for review")
        .bind(STATUS_IN_REVIEW)
        .bind("Ada")
        .execute(&pool)
        .await
        .unwrap();

        let row = fetch_document_row(&pool, result.last_insert_rowid())
            .await
            .unwrap();
        let report = rebuild_document_row(&row);

        assert_eq!(report.matched_attempt().unwrap().validator, "is_in_review");
        assert!(
            report
                .attempts
                .iter()
                .any(|attempt| { attempt.validator == "is_draft" && !attempt.matched })
        );

        match report.into_result().unwrap() {
            document_machine::SomeState::InReview(machine) => {
                assert_eq!(machine.id, 1);
                assert_eq!(machine.title.as_str(), "RFC");
                assert_eq!(machine.body.as_str(), "ready for review");
                assert_eq!(machine.state_data.reviewer.as_str(), "Ada");
            }
            _ => panic!("expected an in-review machine"),
        }
    }
}
