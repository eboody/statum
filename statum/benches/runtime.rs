#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use statum::{machine, state, transition, validators, MachineIntrospection};
use std::hint::black_box;

mod transition_case {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewData {
        reviewer: u64,
    }

    #[state]
    pub enum BenchTransitionState {
        Draft,
        Review(ReviewData),
        Published,
    }

    #[machine]
    pub struct BenchTransitionMachine<BenchTransitionState> {
        ticket: u64,
    }

    #[transition]
    impl BenchTransitionMachine<Draft> {
        fn submit(self, reviewer: u64) -> BenchTransitionMachine<Review> {
            self.transition_with(ReviewData { reviewer })
        }
    }

    #[transition]
    impl BenchTransitionMachine<Review> {
        fn publish(self) -> BenchTransitionMachine<Published> {
            self.transition()
        }
    }

    pub struct PlainDraft {
        ticket: u64,
    }

    pub struct PlainReview {
        ticket: u64,
        state_data: ReviewData,
    }

    pub struct PlainPublished {
        ticket: u64,
    }

    impl PlainDraft {
        fn submit(self, reviewer: u64) -> PlainReview {
            PlainReview {
                ticket: self.ticket,
                state_data: ReviewData { reviewer },
            }
        }
    }

    impl PlainReview {
        fn publish(self) -> PlainPublished {
            PlainPublished {
                ticket: self.ticket,
            }
        }
    }

    pub fn statum_chain(ticket: u64, reviewer: u64) -> u64 {
        let machine = BenchTransitionMachine::<Draft>::builder()
            .ticket(ticket)
            .build();
        let machine = machine.submit(reviewer);
        let machine = machine.publish();
        machine.ticket
    }

    pub fn plain_chain(ticket: u64, reviewer: u64) -> u64 {
        let machine = PlainDraft { ticket };
        let machine = machine.submit(reviewer);
        let machine = machine.publish();
        machine.ticket
    }
}

mod rebuild_case {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload {
        reviewer: u64,
    }

    #[state]
    pub enum BenchRebuildState {
        Draft,
        Review(ReviewPayload),
        Done,
    }

    #[machine]
    pub struct BenchRebuildMachine<BenchRebuildState> {
        tenant: u64,
        shard: u64,
    }

    #[derive(Clone)]
    pub struct PersistedRow {
        pub status: u8,
        pub reviewer: u64,
    }

    #[validators(BenchRebuildMachine)]
    impl PersistedRow {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_done(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    #[derive(Clone)]
    pub struct PlainDraftMachine {
        tenant: u64,
        shard: u64,
    }

    #[derive(Clone)]
    pub struct PlainReviewMachine {
        tenant: u64,
        shard: u64,
        state_data: ReviewPayload,
    }

    #[derive(Clone)]
    pub struct PlainDoneMachine {
        tenant: u64,
        shard: u64,
    }

    pub enum PlainMachineState {
        Draft(PlainDraftMachine),
        Review(PlainReviewMachine),
        Done(PlainDoneMachine),
    }

    pub struct PlainRebuildAttempt {
        matched: bool,
    }

    pub struct PlainRebuildReport {
        attempts: Vec<PlainRebuildAttempt>,
        matched: bool,
    }

    pub fn sample_row() -> PersistedRow {
        PersistedRow {
            status: 1,
            reviewer: 7,
        }
    }

    pub fn sample_rows(count: usize) -> Vec<PersistedRow> {
        (0..count)
            .map(|idx| PersistedRow {
                status: (idx % 3) as u8,
                reviewer: idx as u64,
            })
            .collect()
    }

    pub fn statum_rebuild_tag(row: PersistedRow, tenant: u64, shard: u64) -> u8 {
        let state = row
            .into_machine()
            .tenant(tenant)
            .shard(shard)
            .build()
            .unwrap();

        match state {
            bench_rebuild_machine::SomeState::Draft(_) => 0,
            bench_rebuild_machine::SomeState::Review(_) => 1,
            bench_rebuild_machine::SomeState::Done(_) => 2,
        }
    }

    pub fn plain_rebuild_tag(row: &PersistedRow, tenant: u64, shard: u64) -> u8 {
        match plain_rebuild(row, tenant, shard).unwrap() {
            PlainMachineState::Draft(_) => 0,
            PlainMachineState::Review(_) => 1,
            PlainMachineState::Done(_) => 2,
        }
    }

    pub fn statum_build_report_summary(
        row: PersistedRow,
        tenant: u64,
        shard: u64,
    ) -> (usize, bool) {
        let report = row
            .into_machine()
            .tenant(tenant)
            .shard(shard)
            .build_report();
        (report.attempts.len(), report.result.is_ok())
    }

    pub fn plain_build_report_summary(
        row: &PersistedRow,
        tenant: u64,
        shard: u64,
    ) -> (usize, bool) {
        let report = plain_build_report(row, tenant, shard);
        (report.attempts.len(), report.matched)
    }

    pub fn statum_batch_summary(
        rows: Vec<PersistedRow>,
        tenant: u64,
        shard: u64,
    ) -> (usize, usize) {
        let results = rows.into_machines().tenant(tenant).shard(shard).build();
        let matched = results.iter().filter(|result| result.is_ok()).count();
        (results.len(), matched)
    }

    pub fn plain_batch_summary(rows: Vec<PersistedRow>, tenant: u64, shard: u64) -> (usize, usize) {
        let results = rows
            .iter()
            .map(|row| plain_rebuild(row, tenant, shard))
            .collect::<Vec<_>>();
        let matched = results.iter().filter(|result| result.is_ok()).count();
        (results.len(), matched)
    }

    fn plain_rebuild(
        row: &PersistedRow,
        tenant: u64,
        shard: u64,
    ) -> Result<PlainMachineState, statum::Error> {
        match row.status {
            0 => Ok(PlainMachineState::Draft(PlainDraftMachine {
                tenant,
                shard,
            })),
            1 => Ok(PlainMachineState::Review(PlainReviewMachine {
                tenant,
                shard,
                state_data: ReviewPayload {
                    reviewer: row.reviewer,
                },
            })),
            2 => Ok(PlainMachineState::Done(PlainDoneMachine { tenant, shard })),
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn plain_build_report(row: &PersistedRow, tenant: u64, shard: u64) -> PlainRebuildReport {
        let mut attempts = Vec::with_capacity(3);

        attempts.push(PlainRebuildAttempt {
            matched: row.status == 0,
        });
        if row.status == 0 {
            let _ = plain_rebuild(row, tenant, shard);
            return PlainRebuildReport {
                attempts,
                matched: true,
            };
        }

        attempts.push(PlainRebuildAttempt {
            matched: row.status == 1,
        });
        if row.status == 1 {
            let _ = plain_rebuild(row, tenant, shard);
            return PlainRebuildReport {
                attempts,
                matched: true,
            };
        }

        attempts.push(PlainRebuildAttempt {
            matched: row.status == 2,
        });
        let matched = row.status == 2;
        let _ = plain_rebuild(row, tenant, shard);

        PlainRebuildReport { attempts, matched }
    }
}

mod introspection_case {
    use super::*;

    #[state]
    pub enum BenchIntrospectionState {
        Draft,
        Review,
        Accepted,
        Rejected,
    }

    #[machine]
    pub struct BenchIntrospectionMachine<BenchIntrospectionState> {}

    #[transition]
    impl BenchIntrospectionMachine<Draft> {
        fn submit(self) -> BenchIntrospectionMachine<Review> {
            self.transition()
        }
    }

    #[transition]
    impl BenchIntrospectionMachine<Review> {
        fn decide(
            self,
        ) -> ::core::result::Result<
            BenchIntrospectionMachine<Accepted>,
            BenchIntrospectionMachine<Rejected>,
        > {
            if true {
                Ok(self.accept())
            } else {
                Err(self.reject())
            }
        }

        fn accept(self) -> BenchIntrospectionMachine<Accepted> {
            self.transition()
        }

        fn reject(self) -> BenchIntrospectionMachine<Rejected> {
            self.transition()
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub enum PlainStateId {
        Draft,
        Review,
        Accepted,
        Rejected,
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub enum PlainTransitionId {
        Submit,
        Decide,
    }

    pub struct PlainTransitionDesc {
        id: PlainTransitionId,
        to: &'static [PlainStateId],
    }

    static PLAIN_SUBMIT_TARGETS: [PlainStateId; 1] = [PlainStateId::Review];
    static PLAIN_DECIDE_TARGETS: [PlainStateId; 2] =
        [PlainStateId::Accepted, PlainStateId::Rejected];
    static PLAIN_TRANSITIONS: [PlainTransitionDesc; 2] = [
        PlainTransitionDesc {
            id: PlainTransitionId::Submit,
            to: &PLAIN_SUBMIT_TARGETS,
        },
        PlainTransitionDesc {
            id: PlainTransitionId::Decide,
            to: &PLAIN_DECIDE_TARGETS,
        },
    ];

    pub fn statum_legal_targets_len(
        transition_id: bench_introspection_machine::TransitionId,
    ) -> usize {
        let graph = <BenchIntrospectionMachine<Draft> as MachineIntrospection>::GRAPH;
        graph
            .legal_targets(transition_id)
            .map(|targets| targets.len())
            .unwrap_or(0)
    }

    pub fn statum_transition_lookup(
        transition_id: bench_introspection_machine::TransitionId,
    ) -> bool {
        let graph = <BenchIntrospectionMachine<Draft> as MachineIntrospection>::GRAPH;
        graph.transition(transition_id).is_some()
    }

    pub fn plain_legal_targets_len(transition_id: PlainTransitionId) -> usize {
        plain_transition(transition_id)
            .map(|transition| transition.to.len())
            .unwrap_or(0)
    }

    pub fn plain_transition_lookup(transition_id: PlainTransitionId) -> bool {
        plain_transition(transition_id).is_some()
    }

    fn plain_transition(id: PlainTransitionId) -> Option<&'static PlainTransitionDesc> {
        PLAIN_TRANSITIONS
            .iter()
            .find(|transition| transition.id == id)
    }
}

fn bench_transition_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("transition_chain");

    group.bench_function("statum", |b| {
        b.iter(|| black_box(transition_case::statum_chain(black_box(7), black_box(11))))
    });

    group.bench_function("plain", |b| {
        b.iter(|| black_box(transition_case::plain_chain(black_box(7), black_box(11))))
    });

    group.finish();
}

fn bench_rebuild_single(c: &mut Criterion) {
    let row = rebuild_case::sample_row();
    let mut group = c.benchmark_group("rebuild_single");

    group.bench_function("statum", |b| {
        b.iter_batched(
            || row.clone(),
            |row| black_box(rebuild_case::statum_rebuild_tag(row, 7, 3)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("plain", |b| {
        b.iter_batched(
            || row.clone(),
            |row| black_box(rebuild_case::plain_rebuild_tag(&row, 7, 3)),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_rebuild_report(c: &mut Criterion) {
    let row = rebuild_case::sample_row();
    let mut group = c.benchmark_group("rebuild_report");

    group.bench_function("statum", |b| {
        b.iter_batched(
            || row.clone(),
            |row| black_box(rebuild_case::statum_build_report_summary(row, 7, 3)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("plain", |b| {
        b.iter_batched(
            || row.clone(),
            |row| black_box(rebuild_case::plain_build_report_summary(&row, 7, 3)),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_rebuild_batch(c: &mut Criterion) {
    let rows = rebuild_case::sample_rows(64);
    let mut group = c.benchmark_group("rebuild_batch");

    group.bench_function("statum", |b| {
        b.iter_batched(
            || rows.clone(),
            |rows| black_box(rebuild_case::statum_batch_summary(rows, 7, 3)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("plain", |b| {
        b.iter_batched(
            || rows.clone(),
            |rows| black_box(rebuild_case::plain_batch_summary(rows, 7, 3)),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_introspection_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("introspection_queries");

    group.bench_function("statum_legal_targets", |b| {
        b.iter(|| {
            black_box(introspection_case::statum_legal_targets_len(black_box(
                introspection_case::BenchIntrospectionMachine::<introspection_case::Draft>::SUBMIT,
            )))
        })
    });
    group.bench_function("plain_legal_targets", |b| {
        b.iter(|| {
            black_box(introspection_case::plain_legal_targets_len(black_box(
                introspection_case::PlainTransitionId::Submit,
            )))
        })
    });

    group.bench_function("statum_transition_lookup", |b| {
        b.iter(|| {
            black_box(introspection_case::statum_transition_lookup(black_box(
                introspection_case::BenchIntrospectionMachine::<introspection_case::Review>::DECIDE,
            )))
        })
    });
    group.bench_function("plain_transition_lookup", |b| {
        b.iter(|| {
            black_box(introspection_case::plain_transition_lookup(black_box(
                introspection_case::PlainTransitionId::Decide,
            )))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_transition_chain,
    bench_rebuild_single,
    bench_rebuild_report,
    bench_rebuild_batch,
    bench_introspection_queries
);
criterion_main!(benches);
