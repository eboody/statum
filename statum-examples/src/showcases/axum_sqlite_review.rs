use axum::{
    Json, Router,
    extract::{Path, State},
    http, response as reply,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, sqlite};
use statum::{machine, state, transition, validators};

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

impl DocumentRow {
    fn into_response(self) -> DocumentResponse {
        DocumentResponse {
            id: self.id,
            title: self.title,
            body: self.body,
            status: self.status,
            reviewer: self.reviewer,
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

impl AppError {
    fn invalid_transition(message: &'static str) -> Self {
        Self::InvalidTransition(message)
    }
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
    Ok((http::StatusCode::CREATED, Json(row.into_response())))
}

async fn get_document(
    State(app): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DocumentResponse>, AppError> {
    let row = fetch_document_row(&app.pool, id).await?;
    Ok(Json(row.into_response()))
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
            return Err(AppError::invalid_transition(
                "submit requires a draft document",
            ));
        }
    };

    persist_in_review(&app.pool, &machine).await?;

    let row = fetch_document_row(&app.pool, id).await?;
    Ok(Json(row.into_response()))
}

async fn approve_document(
    State(app): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DocumentResponse>, AppError> {
    let machine = load_document_state(&app.pool, id).await?;

    let machine = match machine {
        document_machine::SomeState::InReview(machine) => machine.approve(),
        _ => {
            return Err(AppError::invalid_transition(
                "approve requires an in-review document",
            ));
        }
    };

    persist_published(&app.pool, &machine).await?;

    let row = fetch_document_row(&app.pool, id).await?;
    Ok(Json(row.into_response()))
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

    row.clone()
        .into_machine()
        .id(row.id)
        .title(row.title)
        .body(row.body)
        .build()
        .map_err(|_| AppError::CorruptState)
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

    #[tokio::test]
    async fn row_rehydrates_into_draft_machine_state() {
        let pool = build_pool().await.unwrap();
        init_schema(&pool).await.unwrap();

        let result = sqlx::query(
            r#"
            INSERT INTO documents (title, body, status, reviewer)
            VALUES (?, ?, ?, NULL)
            "#,
        )
        .bind("RFC")
        .bind("draft body")
        .bind(STATUS_DRAFT)
        .execute(&pool)
        .await
        .unwrap();

        let state = load_document_state(&pool, result.last_insert_rowid())
            .await
            .unwrap();

        match state {
            document_machine::SomeState::Draft(machine) => {
                assert_eq!(machine.id, 1);
                assert_eq!(machine.title.as_str(), "RFC");
                assert_eq!(machine.body.as_str(), "draft body");
            }
            _ => panic!("expected a draft machine"),
        }
    }
}
