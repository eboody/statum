#![allow(dead_code)]

use std::borrow::Cow;

use statum::{machine, state, validators, Error, RebuildAttempt};

fn plain_attempt(
    validator: &'static str,
    target_state: &'static str,
    matched: bool,
) -> RebuildAttempt {
    RebuildAttempt {
        validator,
        target_state,
        matched,
        reason_key: None,
        message: None,
    }
}

fn diagnostic_attempt(
    validator: &'static str,
    target_state: &'static str,
    reason_key: &'static str,
    message: &'static str,
) -> RebuildAttempt {
    RebuildAttempt {
        validator,
        target_state,
        matched: false,
        reason_key: Some(reason_key),
        message: Some(Cow::Borrowed(message)),
    }
}

mod sync_reports {
    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct ReviewPayload {
        pub reviewer: String,
    }

    #[state]
    pub enum SyncReportState {
        DraftSync,
        ReviewSync(ReviewPayload),
        DoneSync,
    }

    #[machine]
    pub struct SyncReportMachine<SyncReportState> {
        pub name: String,
    }

    pub struct SyncRow {
        pub status: &'static str,
        pub reviewer: Option<&'static str>,
        pub name: &'static str,
    }

    #[validators(SyncReportMachine)]
    impl SyncRow {
        fn is_draft_sync(&self) -> statum::Result<()> {
            let _ = &name;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_review_sync(&self) -> statum::Result<ReviewPayload> {
            let _ = &name;
            if self.status == "review" {
                Ok(ReviewPayload {
                    reviewer: self.reviewer.expect("reviewer").to_owned(),
                })
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_done_sync(&self) -> statum::Result<()> {
            let _ = &name;
            if self.status == "done" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }
    }
}

mod async_reports {
    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct RunningPayload {
        pub worker_id: u64,
    }

    #[state]
    pub enum AsyncReportState {
        QueuedAsync,
        RunningAsync(RunningPayload),
        CompleteAsync,
    }

    #[machine]
    pub struct AsyncReportMachine<AsyncReportState> {
        pub worker: String,
    }

    pub struct AsyncRow {
        pub status: &'static str,
    }

    #[validators(AsyncReportMachine)]
    impl AsyncRow {
        async fn is_queued_async(&self) -> statum::Result<()> {
            let _ = &worker;
            if self.status == "queued" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }

        async fn is_running_async(&self) -> statum::Result<RunningPayload> {
            let _ = &worker;
            if self.status == "running" {
                Ok(RunningPayload { worker_id: 7 })
            } else {
                Err(Error::InvalidState)
            }
        }

        async fn is_complete_async(&self) -> statum::Result<()> {
            let _ = &worker;
            if self.status == "complete" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }
    }
}

mod diagnostic_reports {
    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct ReviewPayload {
        pub reviewer: String,
    }

    #[state]
    pub enum DiagnosticReportState {
        DraftDiagnostic,
        ReviewDiagnostic(ReviewPayload),
        DoneDiagnostic,
    }

    #[machine]
    pub struct DiagnosticReportMachine<DiagnosticReportState> {
        pub name: String,
    }

    pub struct DiagnosticRow {
        pub status: &'static str,
        pub reviewer: Option<&'static str>,
        pub name: &'static str,
    }

    #[validators(DiagnosticReportMachine)]
    impl DiagnosticRow {
        fn is_draft_diagnostic(&self) -> statum::Validation<()> {
            let _ = &name;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum::Rejection::new("wrong_status").with_message("expected draft status"))
            }
        }

        fn is_review_diagnostic(&self) -> core::result::Result<ReviewPayload, statum::Rejection> {
            let _ = &name;
            if self.status != "review" {
                return Err(
                    statum::Rejection::new("wrong_status").with_message("expected review status")
                );
            }

            self.reviewer
                .map(|reviewer| ReviewPayload {
                    reviewer: reviewer.to_owned(),
                })
                .ok_or_else(|| {
                    statum::Rejection::new("missing_reviewer")
                        .with_message("review rows need a reviewer")
                })
        }

        fn is_done_diagnostic(&self) -> statum::Result<()> {
            let _ = &name;
            if self.status == "done" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }
    }
}

mod async_diagnostic_reports {
    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct RunningPayload {
        pub worker_id: u64,
    }

    #[state]
    pub enum AsyncDiagnosticReportState {
        QueuedAsyncDiagnostic,
        RunningAsyncDiagnostic(RunningPayload),
    }

    #[machine]
    pub struct AsyncDiagnosticReportMachine<AsyncDiagnosticReportState> {
        pub worker: String,
    }

    pub struct AsyncDiagnosticRow {
        pub status: &'static str,
    }

    #[validators(AsyncDiagnosticReportMachine)]
    impl AsyncDiagnosticRow {
        async fn is_queued_async_diagnostic(&self) -> statum::Validation<()> {
            let _ = &worker;
            if self.status == "queued" {
                Ok(())
            } else {
                Err(statum::Rejection::new("wrong_status").with_message("expected queued status"))
            }
        }

        async fn is_running_async_diagnostic(&self) -> statum::Validation<RunningPayload> {
            let _ = &worker;
            if self.status == "running" {
                Ok(RunningPayload { worker_id: 42 })
            } else {
                Err(statum::Rejection::new("wrong_status").with_message("expected running status"))
            }
        }
    }
}

#[test]
fn build_report_records_attempts_for_single_sync_match() {
    let row = sync_reports::SyncRow {
        status: "review",
        reviewer: Some("alice"),
        name: "spec",
    };
    let report = row.into_machine().name(row.name.to_owned()).build_report();

    assert_eq!(
        report.attempts,
        vec![
            plain_attempt("is_draft_sync", "DraftSync", false),
            plain_attempt("is_review_sync", "ReviewSync", true),
        ]
    );
    assert_eq!(
        report.matched_attempt(),
        Some(&plain_attempt("is_review_sync", "ReviewSync", true))
    );

    match report.into_result().unwrap() {
        sync_reports::sync_report_machine::SomeState::ReviewSync(machine) => {
            assert_eq!(machine.name, "spec");
            assert_eq!(
                machine.state_data,
                sync_reports::ReviewPayload {
                    reviewer: "alice".to_owned(),
                }
            );
        }
        _ => panic!("expected review state"),
    }
}

#[test]
fn build_report_records_failed_attempts_for_single_sync_miss() {
    let row = sync_reports::SyncRow {
        status: "missing",
        reviewer: None,
        name: "spec",
    };
    let report = row.into_machine().name(row.name.to_owned()).build_report();

    assert_eq!(
        report.attempts,
        vec![
            plain_attempt("is_draft_sync", "DraftSync", false),
            plain_attempt("is_review_sync", "ReviewSync", false),
            plain_attempt("is_done_sync", "DoneSync", false),
        ]
    );
    assert!(report.matched_attempt().is_none());
    assert!(matches!(report.into_result(), Err(Error::InvalidState)));
}

#[test]
fn build_reports_preserves_sync_batch_order() {
    use sync_reports::sync_report_machine::IntoMachinesExt as _;

    let reports = vec![
        sync_reports::SyncRow {
            status: "draft",
            reviewer: None,
            name: "shared",
        },
        sync_reports::SyncRow {
            status: "done",
            reviewer: None,
            name: "shared",
        },
    ]
    .into_machines()
    .name("shared".to_owned())
    .build_reports();

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].attempts[0].validator, "is_draft_sync");
    assert!(reports[0].attempts[0].matched);
    assert_eq!(
        reports[1].attempts.last().unwrap().validator,
        "is_done_sync"
    );
    assert!(reports[1].attempts.last().unwrap().matched);
}

#[test]
fn build_reports_by_preserves_per_item_fields() {
    use sync_reports::sync_report_machine::IntoMachinesExt as _;

    let reports = vec![
        sync_reports::SyncRow {
            status: "draft",
            reviewer: None,
            name: "first",
        },
        sync_reports::SyncRow {
            status: "review",
            reviewer: Some("bob"),
            name: "second",
        },
    ]
    .into_machines_by(|row| sync_reports::sync_report_machine::Fields {
        name: row.name.to_owned(),
    })
    .build_reports();

    match reports[0].result.as_ref().unwrap() {
        sync_reports::sync_report_machine::SomeState::DraftSync(machine) => {
            assert_eq!(machine.name, "first");
        }
        _ => panic!("expected draft state"),
    }

    match reports[1].result.as_ref().unwrap() {
        sync_reports::sync_report_machine::SomeState::ReviewSync(machine) => {
            assert_eq!(machine.name, "second");
            assert_eq!(machine.state_data.reviewer, "bob");
        }
        _ => panic!("expected review state"),
    }
}

#[tokio::test]
async fn build_report_supports_async_validators() {
    let row = async_reports::AsyncRow { status: "running" };
    let report = row
        .into_machine()
        .worker("worker-7".to_owned())
        .build_report()
        .await;

    assert_eq!(
        report.attempts,
        vec![
            plain_attempt("is_queued_async", "QueuedAsync", false),
            plain_attempt("is_running_async", "RunningAsync", true),
        ]
    );

    match report.into_result().unwrap() {
        async_reports::async_report_machine::SomeState::RunningAsync(machine) => {
            assert_eq!(machine.worker, "worker-7");
            assert_eq!(
                machine.state_data,
                async_reports::RunningPayload { worker_id: 7 }
            );
        }
        _ => panic!("expected running state"),
    }
}

#[test]
fn build_report_captures_diagnostic_reasons_for_sync_miss() {
    let row = diagnostic_reports::DiagnosticRow {
        status: "missing",
        reviewer: None,
        name: "spec",
    };
    let report = row.into_machine().name(row.name.to_owned()).build_report();

    assert_eq!(
        report.attempts,
        vec![
            diagnostic_attempt(
                "is_draft_diagnostic",
                "DraftDiagnostic",
                "wrong_status",
                "expected draft status",
            ),
            diagnostic_attempt(
                "is_review_diagnostic",
                "ReviewDiagnostic",
                "wrong_status",
                "expected review status",
            ),
            plain_attempt("is_done_diagnostic", "DoneDiagnostic", false),
        ]
    );
    assert!(report.matched_attempt().is_none());
    assert!(matches!(report.into_result(), Err(Error::InvalidState)));
}

#[test]
fn build_report_preserves_first_match_with_mixed_validator_styles() {
    let row = diagnostic_reports::DiagnosticRow {
        status: "review",
        reviewer: Some("alice"),
        name: "doc",
    };
    let report = row.into_machine().name(row.name.to_owned()).build_report();

    assert_eq!(
        report.attempts,
        vec![
            diagnostic_attempt(
                "is_draft_diagnostic",
                "DraftDiagnostic",
                "wrong_status",
                "expected draft status",
            ),
            plain_attempt("is_review_diagnostic", "ReviewDiagnostic", true),
        ]
    );

    match report.into_result().unwrap() {
        diagnostic_reports::diagnostic_report_machine::SomeState::ReviewDiagnostic(machine) => {
            assert_eq!(machine.name, "doc");
            assert_eq!(
                machine.state_data,
                diagnostic_reports::ReviewPayload {
                    reviewer: "alice".to_owned(),
                }
            );
        }
        _ => panic!("expected review state"),
    }
}

#[test]
fn build_reports_preserve_per_item_diagnostic_reasons() {
    use diagnostic_reports::diagnostic_report_machine::IntoMachinesExt as _;

    let reports = vec![
        diagnostic_reports::DiagnosticRow {
            status: "missing",
            reviewer: None,
            name: "first",
        },
        diagnostic_reports::DiagnosticRow {
            status: "done",
            reviewer: None,
            name: "second",
        },
    ]
    .into_machines_by(
        |row| diagnostic_reports::diagnostic_report_machine::Fields {
            name: row.name.to_owned(),
        },
    )
    .build_reports();

    assert_eq!(
        reports[0].attempts[0],
        diagnostic_attempt(
            "is_draft_diagnostic",
            "DraftDiagnostic",
            "wrong_status",
            "expected draft status",
        )
    );
    assert!(reports[0].result.is_err());
    assert_eq!(
        reports[1].matched_attempt(),
        Some(&plain_attempt("is_done_diagnostic", "DoneDiagnostic", true))
    );
}

#[tokio::test]
async fn build_report_captures_diagnostic_reasons_for_async_validators() {
    let row = async_diagnostic_reports::AsyncDiagnosticRow { status: "missing" };
    let report = row
        .into_machine()
        .worker("worker-42".to_owned())
        .build_report()
        .await;

    assert_eq!(
        report.attempts,
        vec![
            diagnostic_attempt(
                "is_queued_async_diagnostic",
                "QueuedAsyncDiagnostic",
                "wrong_status",
                "expected queued status",
            ),
            diagnostic_attempt(
                "is_running_async_diagnostic",
                "RunningAsyncDiagnostic",
                "wrong_status",
                "expected running status",
            ),
        ]
    );
    assert!(matches!(report.into_result(), Err(Error::InvalidState)));
}
