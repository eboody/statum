//! Rehydration fixture assertions for persisted row, snapshot, and event tests.
//!
//! These helpers inspect an already-built [`RebuildReport`].
//! They do not rerun validators or prove that the backing store is complete.

use crate::{RebuildAmbiguity, RebuildReport};
use core::fmt;

/// The persisted input shape represented by a rehydration fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebuildFixtureKind {
    /// A database row or row-like record.
    Row,
    /// A serialized snapshot document.
    Snapshot,
    /// A projected event-log state.
    Event,
}

impl RebuildFixtureKind {
    fn label(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Snapshot => "snapshot",
            Self::Event => "event",
        }
    }
}

/// Wraps a rebuild report with fixture context for assertion messages.
#[derive(Debug)]
pub struct RebuildFixture<M> {
    kind: RebuildFixtureKind,
    report: RebuildReport<M>,
}

/// Creates a persisted-row rebuild fixture assertion.
pub fn row_fixture<M>(report: RebuildReport<M>) -> RebuildFixture<M> {
    RebuildFixture::new(RebuildFixtureKind::Row, report)
}

/// Creates a snapshot rebuild fixture assertion.
pub fn snapshot_fixture<M>(report: RebuildReport<M>) -> RebuildFixture<M> {
    RebuildFixture::new(RebuildFixtureKind::Snapshot, report)
}

/// Creates an event-projection rebuild fixture assertion.
pub fn event_fixture<M>(report: RebuildReport<M>) -> RebuildFixture<M> {
    RebuildFixture::new(RebuildFixtureKind::Event, report)
}

impl<M> RebuildFixture<M> {
    /// Creates a fixture assertion for a custom fixture kind.
    pub fn new(kind: RebuildFixtureKind, report: RebuildReport<M>) -> Self {
        Self { kind, report }
    }

    /// Asserts that the fixture rebuilt into `state`.
    pub fn rebuilds_as(self, state: &'static str) -> SuccessfulRebuildFixture<M> {
        assert!(
            self.report.result.is_ok(),
            "expected {} fixture for {} to rebuild as {}, but rebuild failed after attempts {}",
            self.kind.label(),
            self.report.machine,
            state,
            AttemptList(&self.report.attempts)
        );

        let matched = self.report.matched_attempt().unwrap_or_else(|| {
            panic!(
                "expected {} fixture for {} to rebuild as {}, but no matched validator attempt was recorded; attempts {}",
                self.kind.label(),
                self.report.machine,
                state,
                AttemptList(&self.report.attempts)
            )
        });
        assert_eq!(
            matched.target_state,
            state,
            "expected {} fixture for {} to rebuild as {}, but matched validator {} rebuilt as {}",
            self.kind.label(),
            self.report.machine,
            state,
            matched.validator,
            matched.target_state
        );

        SuccessfulRebuildFixture { fixture: self }
    }

    /// Asserts that the fixture failed to rebuild into any state.
    pub fn fails(self) -> FailedRebuildFixture<M> {
        assert!(
            self.report.result.is_err(),
            "expected {} fixture for {} to fail, but matched {:?}",
            self.kind.label(),
            self.report.machine,
            self.report
                .matched_attempt()
                .map(|attempt| attempt.target_state)
        );

        FailedRebuildFixture { fixture: self }
    }
}

/// Follow-up assertions for a fixture that rebuilt successfully.
#[derive(Debug)]
pub struct SuccessfulRebuildFixture<M> {
    fixture: RebuildFixture<M>,
}

impl<M> SuccessfulRebuildFixture<M> {
    /// Asserts that the named validator was the matching attempt.
    pub fn matched_by(self, validator: &'static str) -> Self {
        let matched = self.fixture.report.matched_attempt().unwrap_or_else(|| {
            panic!(
                "expected {} fixture for {} to be matched by {}, but no matched validator attempt was recorded",
                self.fixture.kind.label(),
                self.fixture.report.machine,
                validator
            )
        });
        assert_eq!(
            matched.validator,
            validator,
            "expected {} fixture for {} to be matched by {}, but matched validator was {}",
            self.fixture.kind.label(),
            self.fixture.report.machine,
            validator,
            matched.validator
        );
        self
    }

    /// Returns the wrapped report for callers that want additional custom assertions.
    pub fn into_report(self) -> RebuildReport<M> {
        self.fixture.report
    }
}

/// Follow-up assertions for a fixture that failed to rebuild.
#[derive(Debug)]
pub struct FailedRebuildFixture<M> {
    fixture: RebuildFixture<M>,
}

impl<M> FailedRebuildFixture<M> {
    /// Asserts that the report considered exactly these candidate states in order.
    pub fn candidate_states<I>(self, expected: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        let expected = expected.into_iter().collect::<Vec<_>>();
        assert_eq!(
            self.fixture.report.candidate_states,
            expected,
            "expected {} fixture for {} to consider candidate states {:?}, but candidates were {:?}",
            self.fixture.kind.label(),
            self.fixture.report.machine,
            expected,
            self.fixture.report.candidate_states
        );
        self
    }

    /// Asserts that the report checked all candidates and found at most one match.
    pub fn unambiguous(self) -> Self {
        assert_eq!(
            self.fixture.report.ambiguity,
            RebuildAmbiguity::Unambiguous,
            "expected {} fixture for {} to have unambiguous rebuild evidence, but ambiguity was {:?}",
            self.fixture.kind.label(),
            self.fixture.report.machine,
            self.fixture.report.ambiguity
        );
        self
    }

    /// Asserts that the report checked all candidates and found more than one matching state.
    pub fn ambiguous_between<I>(self, expected: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        let expected = expected.into_iter().collect::<Vec<_>>();
        match &self.fixture.report.ambiguity {
            RebuildAmbiguity::Ambiguous { matched_states } => {
                assert_eq!(
                    matched_states,
                    &expected,
                    "expected {} fixture for {} to be ambiguous between {:?}, but matched states were {:?}",
                    self.fixture.kind.label(),
                    self.fixture.report.machine,
                    expected,
                    matched_states
                );
            }
            ambiguity => panic!(
                "expected {} fixture for {} to have ambiguous rebuild evidence {:?}, but ambiguity was {:?}",
                self.fixture.kind.label(),
                self.fixture.report.machine,
                expected,
                ambiguity
            ),
        }
        self
    }

    /// Asserts that `validator` rejected the fixture with `reason_key`.
    pub fn rejected_by(self, validator: &'static str, reason_key: &'static str) -> Self {
        let attempt = self
            .fixture
            .report
            .attempts
            .iter()
            .find(|attempt| attempt.validator == validator)
            .unwrap_or_else(|| {
                panic!(
                    "expected {} fixture for {} to include rejected validator {}, but attempts were {}",
                    self.fixture.kind.label(),
                    self.fixture.report.machine,
                    validator,
                    AttemptList(&self.fixture.report.attempts)
                )
            });

        assert!(
            !attempt.matched,
            "expected {} fixture for {} validator {} to reject with {}, but it matched {}",
            self.fixture.kind.label(),
            self.fixture.report.machine,
            validator,
            reason_key,
            attempt.target_state
        );
        assert_eq!(
            attempt.reason_key,
            Some(reason_key),
            "expected {} fixture for {} validator {} to reject with reason {}, but reason was {:?}",
            self.fixture.kind.label(),
            self.fixture.report.machine,
            validator,
            reason_key,
            attempt.reason_key
        );
        self
    }

    /// Returns the wrapped report for callers that want additional custom assertions.
    pub fn into_report(self) -> RebuildReport<M> {
        self.fixture.report
    }
}

struct AttemptList<'a>(&'a [crate::RebuildAttempt]);

impl fmt::Display for AttemptList<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("[")?;
        for (index, attempt) in self.0.iter().enumerate() {
            if index > 0 {
                fmt.write_str(", ")?;
            }
            write!(
                fmt,
                "{}:{}:{}",
                attempt.validator,
                attempt.target_state,
                if attempt.matched {
                    "matched"
                } else {
                    "rejected"
                }
            )?;
            if let Some(reason_key) = attempt.reason_key {
                write!(fmt, "({reason_key})")?;
            }
        }
        fmt.write_str("]")
    }
}
