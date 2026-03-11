use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use serde_json::{Value, json};
use statum_examples::showcases::{
    axum_sqlite_review, sqlite_event_log_rebuild, tokio_sqlite_job_runner, tokio_websocket_session,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Output},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::timeout;
use tower::ServiceExt;

#[tokio::test]
async fn axum_sqlite_review_happy_path() {
    let app = axum_sqlite_review::build_app().await.unwrap();

    let created = send_json(
        &app,
        Method::POST,
        "/documents",
        json!({
            "title": "Spec",
            "body": "Ship typed workflows.",
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let created_body = read_json(created).await;
    let id = created_body["id"].as_i64().unwrap();
    assert_eq!(created_body["status"], "draft");
    assert_eq!(created_body["reviewer"], Value::Null);

    let fetched = send_empty(&app, Method::GET, &format!("/documents/{id}")).await;
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(read_json(fetched).await["status"], "draft");

    let submitted = send_json(
        &app,
        Method::POST,
        &format!("/documents/{id}/submit"),
        json!({
            "reviewer": "Ada",
        }),
    )
    .await;
    assert_eq!(submitted.status(), StatusCode::OK);

    let submitted_body = read_json(submitted).await;
    assert_eq!(submitted_body["status"], "in_review");
    assert_eq!(submitted_body["reviewer"], "Ada");

    let approved = send_empty(&app, Method::POST, &format!("/documents/{id}/approve")).await;
    assert_eq!(approved.status(), StatusCode::OK);

    let approved_body = read_json(approved).await;
    assert_eq!(approved_body["status"], "published");
    assert_eq!(approved_body["reviewer"], Value::Null);
}

#[tokio::test]
async fn axum_sqlite_review_rejects_invalid_transition() {
    let app = axum_sqlite_review::build_app().await.unwrap();

    let created = send_json(
        &app,
        Method::POST,
        "/documents",
        json!({
            "title": "Spec",
            "body": "Still draft.",
        }),
    )
    .await;
    let id = read_json(created).await["id"].as_i64().unwrap();

    let response = send_empty(&app, Method::POST, &format!("/documents/{id}/approve")).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(response).await["error"],
        "approve requires an in-review document"
    );
}

#[tokio::test]
async fn axum_sqlite_review_rejects_invalid_input() {
    let app = axum_sqlite_review::build_app().await.unwrap();

    let response = send_json(
        &app,
        Method::POST,
        "/documents",
        json!({
            "title": "   ",
            "body": "still invalid",
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(response).await["error"], "title is required");

    let created = send_json(
        &app,
        Method::POST,
        "/documents",
        json!({
            "title": "Spec",
            "body": "Ship typed workflows.",
        }),
    )
    .await;
    let id = read_json(created).await["id"].as_i64().unwrap();

    let response = send_json(
        &app,
        Method::POST,
        &format!("/documents/{id}/submit"),
        json!({
            "reviewer": "   ",
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(response).await["error"], "reviewer is required");
}

#[tokio::test]
async fn tokio_sqlite_job_runner_processes_success_retry_and_failure() {
    let mut runner = tokio_sqlite_job_runner::build_runner().await.unwrap();

    let ok_id = runner.enqueue("email", "ok", 3).await.unwrap();
    let flaky_id = runner.enqueue("thumbnail", "flaky", 3).await.unwrap();
    let failed_id = runner.enqueue("invoice", "always-fail", 2).await.unwrap();

    runner.run_until_idle().await.unwrap();

    let ok = runner.snapshot(ok_id).await.unwrap();
    assert_eq!(ok.status, "succeeded");
    assert_eq!(ok.attempts, 1);
    assert_eq!(
        ok.result_text.as_deref(),
        Some("email completed on attempt 1")
    );

    let flaky = runner.snapshot(flaky_id).await.unwrap();
    assert_eq!(flaky.status, "succeeded");
    assert_eq!(flaky.attempts, 2);
    assert_eq!(
        flaky.result_text.as_deref(),
        Some("thumbnail recovered on attempt 2")
    );

    let failed = runner.snapshot(failed_id).await.unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.attempts, 2);
    assert_eq!(
        failed.last_error.as_deref(),
        Some("invoice exhausted after 2 attempts")
    );
}

#[tokio::test]
async fn tokio_sqlite_job_runner_waits_for_retry_backoff() {
    let mut runner = tokio_sqlite_job_runner::build_runner().await.unwrap();
    let job_id = runner.enqueue("thumbnail", "flaky", 3).await.unwrap();

    assert!(runner.run_next().await.unwrap());

    let retrying = runner.snapshot(job_id).await.unwrap();
    assert_eq!(retrying.status, "retrying");
    assert_eq!(retrying.attempts, 1);
    assert_eq!(
        retrying.available_at_ms,
        tokio_sqlite_job_runner::RETRY_DELAY_MS
    );
    assert_eq!(
        retrying.last_error.as_deref(),
        Some("temporary upstream timeout")
    );

    assert!(!runner.run_next().await.unwrap());

    runner.advance_time(tokio_sqlite_job_runner::RETRY_DELAY_MS);
    assert!(runner.run_next().await.unwrap());

    let completed = runner.snapshot(job_id).await.unwrap();
    assert_eq!(completed.status, "succeeded");
    assert_eq!(completed.attempts, 2);
}

#[tokio::test]
async fn tokio_sqlite_job_runner_rejects_invalid_job_config() {
    let runner = tokio_sqlite_job_runner::build_runner().await.unwrap();

    let error = runner.enqueue("", "ok", 3).await.unwrap_err();
    assert_eq!(error.to_string(), "job_type is required");

    let error = runner.enqueue("email", "", 3).await.unwrap_err();
    assert_eq!(error.to_string(), "payload is required");

    let error = runner.enqueue("email", "ok", 0).await.unwrap_err();
    assert_eq!(error.to_string(), "max_attempts must be greater than zero");
}

#[tokio::test]
async fn sqlite_event_log_rebuild_batches_append_only_orders() {
    let store = sqlite_event_log_rebuild::build_store().await.unwrap();

    let delivered_id = store.create_order("acme", "widget").await.unwrap();
    store.pay(delivered_id, "pay-001").await.unwrap();
    store.pack(delivered_id, "pick-001").await.unwrap();
    store.ship(delivered_id, "trk-001").await.unwrap();
    store.deliver(delivered_id).await.unwrap();

    let packed_id = store.create_order("globex", "gizmo").await.unwrap();
    store.pay(packed_id, "pay-002").await.unwrap();
    store.pack(packed_id, "pick-002").await.unwrap();

    let states = store.load_all_states().await.unwrap();
    assert_eq!(states.len(), 2);

    match &states[0] {
        sqlite_event_log_rebuild::order_machine::State::Delivered(machine) => {
            assert_eq!(machine.state_data.order.order_id, delivered_id);
            assert_eq!(machine.state_data.tracking_number.as_str(), "trk-001");
        }
        _ => panic!("expected first order to be delivered"),
    }

    match &states[1] {
        sqlite_event_log_rebuild::order_machine::State::Packed(machine) => {
            assert_eq!(machine.state_data.order.order_id, packed_id);
            assert_eq!(machine.state_data.pick_ticket.as_str(), "pick-002");
        }
        _ => panic!("expected second order to be packed"),
    }
}

#[tokio::test]
async fn sqlite_event_log_rebuild_rejects_invalid_transition() {
    let store = sqlite_event_log_rebuild::build_store().await.unwrap();
    let order_id = store.create_order("acme", "widget").await.unwrap();

    let error = store.ship(order_id, "trk-001").await.unwrap_err();
    assert_eq!(error.to_string(), "ship requires a packed order");

    let state = store.load_state(order_id).await.unwrap();
    match state {
        sqlite_event_log_rebuild::order_machine::State::Created(machine) => {
            assert_eq!(machine.state_data.order.order_id, order_id);
        }
        _ => panic!("expected order to remain created"),
    }
}

#[tokio::test]
async fn tokio_websocket_session_happy_path() {
    let mut session = tokio_websocket_session::spawn_session(7, "127.0.0.1:4000");

    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Hello {
            connection_id: 7,
            peer_label: "127.0.0.1:4000".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Authenticate {
            token: "token:alice".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Authenticated {
            user_id: "alice".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Subscribe {
            topic: "deployments".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Subscribed {
            topic: "deployments".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Publish {
            topic: "deployments".to_string(),
            body: "rollout started".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Delivered {
            user_id: "alice".to_string(),
            topic: "deployments".to_string(),
            body: "rollout started".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Close {
            reason: "demo complete".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Bye {
            reason: "demo complete".to_string(),
        }
    );

    session.finish().await.unwrap();
}

#[tokio::test]
async fn tokio_websocket_session_rejects_out_of_order_frames() {
    let mut session = tokio_websocket_session::spawn_session(8, "127.0.0.1:5000");
    let _ = recv_server_frame(&mut session).await;

    session
        .send(tokio_websocket_session::ClientFrame::Publish {
            topic: "deployments".to_string(),
            body: "too early".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Error {
            message: "authenticate before publishing".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Authenticate {
            token: "nope".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Error {
            message: "token must use token:<user>".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Authenticate {
            token: "token:bob".to_string(),
        })
        .await
        .unwrap();
    let _ = recv_server_frame(&mut session).await;

    session
        .send(tokio_websocket_session::ClientFrame::Subscribe {
            topic: "deployments".to_string(),
        })
        .await
        .unwrap();
    let _ = recv_server_frame(&mut session).await;

    session
        .send(tokio_websocket_session::ClientFrame::Publish {
            topic: "billing".to_string(),
            body: "wrong topic".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        recv_server_frame(&mut session).await,
        tokio_websocket_session::ServerFrame::Error {
            message: "publish topic does not match subscription".to_string(),
        }
    );

    session
        .send(tokio_websocket_session::ClientFrame::Close {
            reason: "done".to_string(),
        })
        .await
        .unwrap();
    let _ = recv_server_frame(&mut session).await;
    session.finish().await.unwrap();
}

#[test]
fn clap_sqlite_deploy_pipeline_happy_path_across_invocations() {
    let db_path = unique_cli_db_path("deploy-pipeline-happy");

    let created = run_cli_ok(
        &db_path,
        &[
            "create",
            "--service",
            "api",
            "--env",
            "prod",
            "--version",
            "1.2.3",
        ],
    );
    assert!(created.contains("state=draft"));
    let id = parse_id(&created);

    let planned = run_cli_ok(&db_path, &["plan", &id.to_string()]);
    assert!(planned.contains("state=planned"));
    assert!(planned.contains("plan_digest=plan:api:prod:1.2.3"));

    let awaiting = run_cli_ok(&db_path, &["request-approval", &id.to_string()]);
    assert!(awaiting.contains("state=awaiting_approval"));
    assert!(awaiting.contains("approval_ticket=approval:1:1.2.3"));

    let applying = run_cli_ok(&db_path, &["approve", &id.to_string(), "--by", "alice"]);
    assert!(applying.contains("state=applying"));
    assert!(applying.contains("operation_id=op:1:1.2.3"));
    assert!(applying.contains("approved_by=alice"));

    let applied = run_cli_ok(
        &db_path,
        &["finish-apply", &id.to_string(), "--result", "success"],
    );
    assert!(applied.contains("state=applied"));
    assert!(applied.contains("receipt_id=receipt:1:1.2.3"));

    let shown = run_cli_ok(&db_path, &["show", &id.to_string()]);
    assert!(shown.contains("state=applied"));
    assert!(shown.contains("receipt_id=receipt:1:1.2.3"));

    cleanup_db(&db_path);
}

#[test]
fn clap_sqlite_deploy_pipeline_rejects_invalid_transition() {
    let db_path = unique_cli_db_path("deploy-pipeline-invalid");

    let created = run_cli_ok(
        &db_path,
        &[
            "create",
            "--service",
            "api",
            "--env",
            "prod",
            "--version",
            "1.2.3",
        ],
    );
    let id = parse_id(&created);

    let output = run_cli(
        &db_path,
        &["rollback", &id.to_string(), "--reason", "too soon"],
    );
    assert!(!output.status.success());
    assert_eq!(
        stderr_string(&output),
        "rollback requires an applied deployment"
    );

    cleanup_db(&db_path);
}

#[test]
fn clap_sqlite_deploy_pipeline_handles_failure_path() {
    let db_path = unique_cli_db_path("deploy-pipeline-failure");

    let created = run_cli_ok(
        &db_path,
        &[
            "create",
            "--service",
            "worker",
            "--env",
            "staging",
            "--version",
            "9.0.0",
        ],
    );
    let id = parse_id(&created);

    run_cli_ok(&db_path, &["plan", &id.to_string()]);
    run_cli_ok(&db_path, &["request-approval", &id.to_string()]);
    run_cli_ok(&db_path, &["approve", &id.to_string(), "--by", "bob"]);

    let failed = run_cli_ok(
        &db_path,
        &[
            "finish-apply",
            &id.to_string(),
            "--result",
            "failure",
            "--reason",
            "health checks failed",
        ],
    );
    assert!(failed.contains("state=failed"));
    assert!(failed.contains("error=health checks failed"));

    let shown = run_cli_ok(&db_path, &["show", &id.to_string()]);
    assert!(shown.contains("state=failed"));
    assert!(shown.contains("error=health checks failed"));

    cleanup_db(&db_path);
}

#[test]
fn clap_sqlite_deploy_pipeline_rolls_back_applied_deployment() {
    let db_path = unique_cli_db_path("deploy-pipeline-rollback");

    let created = run_cli_ok(
        &db_path,
        &[
            "create",
            "--service",
            "frontend",
            "--env",
            "prod",
            "--version",
            "4.5.6",
        ],
    );
    let id = parse_id(&created);

    run_cli_ok(&db_path, &["plan", &id.to_string()]);
    run_cli_ok(&db_path, &["request-approval", &id.to_string()]);
    run_cli_ok(&db_path, &["approve", &id.to_string(), "--by", "carol"]);
    run_cli_ok(
        &db_path,
        &["finish-apply", &id.to_string(), "--result", "success"],
    );

    let rolled_back = run_cli_ok(
        &db_path,
        &[
            "rollback",
            &id.to_string(),
            "--reason",
            "rollback requested",
        ],
    );
    assert!(rolled_back.contains("state=rolled_back"));
    assert!(rolled_back.contains("rollback_reason=rollback requested"));

    cleanup_db(&db_path);
}

async fn send_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: Value,
) -> axum::response::Response {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

async fn send_empty(app: &axum::Router, method: Method, uri: &str) -> axum::response::Response {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn run_cli(db_path: &Path, args: &[&str]) -> Output {
    ProcessCommand::new(env!("CARGO_BIN_EXE_clap-sqlite-deploy-pipeline"))
        .arg("--db")
        .arg(db_path)
        .args(args)
        .output()
        .unwrap()
}

fn run_cli_ok(db_path: &Path, args: &[&str]) -> String {
    let output = run_cli(db_path, args);
    if !output.status.success() {
        panic!(
            "command failed with status {:?}: {}",
            output.status.code(),
            stderr_string(&output)
        );
    }

    stdout_string(&output)
}

fn stdout_string(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr_string(output: &Output) -> String {
    String::from_utf8(output.stderr.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn parse_id(stdout: &str) -> i64 {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("id="))
        .unwrap()
        .parse()
        .unwrap()
}

fn unique_cli_db_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("statum-{name}-{unique}.db"))
}

fn cleanup_db(db_path: &Path) {
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(db_path.with_extension("db-shm"));
    let _ = fs::remove_file(db_path.with_extension("db-wal"));
}

async fn recv_server_frame(
    session: &mut tokio_websocket_session::SessionHandle,
) -> tokio_websocket_session::ServerFrame {
    timeout(Duration::from_secs(1), session.recv())
        .await
        .unwrap()
        .unwrap()
}
