#![allow(dead_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlainError {
    InvalidState,
}

macro_rules! define_plain_flow {
    ($module:ident, $payload:ident, $row:ident) => {
        mod $module {
            use super::PlainError;

            #[derive(Clone)]
            pub struct $payload {
                reviewer: u64,
            }

            pub struct DraftMachine {
                tenant: u64,
                shard: u64,
            }

            pub struct ReviewMachine {
                tenant: u64,
                shard: u64,
                state_data: $payload,
            }

            pub struct PublishedMachine {
                tenant: u64,
                shard: u64,
            }

            impl DraftMachine {
                fn review(self, reviewer: u64) -> ReviewMachine {
                    ReviewMachine {
                        tenant: self.tenant,
                        shard: self.shard,
                        state_data: $payload { reviewer },
                    }
                }
            }

            impl ReviewMachine {
                fn publish(self) -> PublishedMachine {
                    PublishedMachine {
                        tenant: self.tenant,
                        shard: self.shard,
                    }
                }
            }

            #[derive(Clone)]
            pub struct $row {
                status: u8,
                reviewer: u64,
            }

            pub enum RebuiltMachine {
                Draft(DraftMachine),
                Review(ReviewMachine),
                Published(PublishedMachine),
            }

            pub struct RebuildAttempt {
                matched: bool,
            }

            pub struct RebuildReport {
                attempts: Vec<RebuildAttempt>,
                matched: bool,
            }

            pub struct TransitionDesc {
                id: u8,
                targets: &'static [u8],
            }

            static REVIEW_TARGETS: [u8; 1] = [1];
            static TRANSITIONS: [TransitionDesc; 1] = [TransitionDesc {
                id: 0,
                targets: &REVIEW_TARGETS,
            }];

            pub fn exercise() {
                let machine = DraftMachine {
                    tenant: 1,
                    shard: 2,
                };
                let machine = machine.review(9).publish();
                let _ = machine.tenant;

                let row = $row {
                    status: 1,
                    reviewer: 7,
                };
                let _ = rebuild(&row, 1, 2);
                let _ = rebuild_report(&row, 1, 2);

                let rows = vec![
                    $row {
                        status: 0,
                        reviewer: 1,
                    },
                    $row {
                        status: 1,
                        reviewer: 2,
                    },
                    $row {
                        status: 2,
                        reviewer: 3,
                    },
                ];
                let _ = rows
                    .iter()
                    .map(|row| rebuild(row, 1, 2))
                    .collect::<Vec<_>>();
                let _ = transition(0).map(|transition| transition.targets);
            }

            fn rebuild(row: &$row, tenant: u64, shard: u64) -> Result<RebuiltMachine, PlainError> {
                match row.status {
                    0 => Ok(RebuiltMachine::Draft(DraftMachine { tenant, shard })),
                    1 => Ok(RebuiltMachine::Review(ReviewMachine {
                        tenant,
                        shard,
                        state_data: $payload {
                            reviewer: row.reviewer,
                        },
                    })),
                    2 => Ok(RebuiltMachine::Published(PublishedMachine {
                        tenant,
                        shard,
                    })),
                    _ => Err(PlainError::InvalidState),
                }
            }

            fn rebuild_report(row: &$row, tenant: u64, shard: u64) -> RebuildReport {
                let mut attempts = Vec::with_capacity(3);
                attempts.push(RebuildAttempt {
                    matched: row.status == 0,
                });
                if row.status == 0 {
                    let _ = rebuild(row, tenant, shard);
                    return RebuildReport {
                        attempts,
                        matched: true,
                    };
                }

                attempts.push(RebuildAttempt {
                    matched: row.status == 1,
                });
                if row.status == 1 {
                    let _ = rebuild(row, tenant, shard);
                    return RebuildReport {
                        attempts,
                        matched: true,
                    };
                }

                attempts.push(RebuildAttempt {
                    matched: row.status == 2,
                });
                let _ = rebuild(row, tenant, shard);

                RebuildReport {
                    attempts,
                    matched: row.status == 2,
                }
            }

            fn transition(id: u8) -> Option<&'static TransitionDesc> {
                TRANSITIONS.iter().find(|transition| transition.id == id)
            }
        }
    };
}

define_plain_flow!(flow01, ReviewPayload01, CompileRow01);
define_plain_flow!(flow02, ReviewPayload02, CompileRow02);
define_plain_flow!(flow03, ReviewPayload03, CompileRow03);
define_plain_flow!(flow04, ReviewPayload04, CompileRow04);
define_plain_flow!(flow05, ReviewPayload05, CompileRow05);
define_plain_flow!(flow06, ReviewPayload06, CompileRow06);
define_plain_flow!(flow07, ReviewPayload07, CompileRow07);
define_plain_flow!(flow08, ReviewPayload08, CompileRow08);

pub fn exercise_all() {
    flow01::exercise();
    flow02::exercise();
    flow03::exercise();
    flow04::exercise();
    flow05::exercise();
    flow06::exercise();
    flow07::exercise();
    flow08::exercise();
}
