#![allow(dead_code)]

use statum::{MachineIntrospection, machine, state, transition, validators};

mod flow01 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload01 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState01 {
        Draft,
        Review(ReviewPayload01),
        Published,
    }

    #[machine]
    pub struct CompileMachine01<CompileState01> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine01<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine01<Review> {
            self.transition_with(ReviewPayload01 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine01<Review> {
        fn publish(self) -> CompileMachine01<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow01 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine01)]
    impl CompileRow01 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload01> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload01 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine01::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow01 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow01 {
                status: 0,
                reviewer: 1,
            },
            CompileRow01 {
                status: 1,
                reviewer: 2,
            },
            CompileRow01 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine01<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine01::<Draft>::REVIEW);
    }
}

mod flow02 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload02 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState02 {
        Draft,
        Review(ReviewPayload02),
        Published,
    }

    #[machine]
    pub struct CompileMachine02<CompileState02> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine02<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine02<Review> {
            self.transition_with(ReviewPayload02 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine02<Review> {
        fn publish(self) -> CompileMachine02<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow02 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine02)]
    impl CompileRow02 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload02> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload02 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine02::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow02 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow02 {
                status: 0,
                reviewer: 1,
            },
            CompileRow02 {
                status: 1,
                reviewer: 2,
            },
            CompileRow02 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine02<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine02::<Draft>::REVIEW);
    }
}

mod flow03 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload03 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState03 {
        Draft,
        Review(ReviewPayload03),
        Published,
    }

    #[machine]
    pub struct CompileMachine03<CompileState03> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine03<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine03<Review> {
            self.transition_with(ReviewPayload03 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine03<Review> {
        fn publish(self) -> CompileMachine03<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow03 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine03)]
    impl CompileRow03 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload03> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload03 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine03::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow03 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow03 {
                status: 0,
                reviewer: 1,
            },
            CompileRow03 {
                status: 1,
                reviewer: 2,
            },
            CompileRow03 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine03<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine03::<Draft>::REVIEW);
    }
}

mod flow04 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload04 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState04 {
        Draft,
        Review(ReviewPayload04),
        Published,
    }

    #[machine]
    pub struct CompileMachine04<CompileState04> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine04<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine04<Review> {
            self.transition_with(ReviewPayload04 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine04<Review> {
        fn publish(self) -> CompileMachine04<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow04 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine04)]
    impl CompileRow04 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload04> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload04 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine04::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow04 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow04 {
                status: 0,
                reviewer: 1,
            },
            CompileRow04 {
                status: 1,
                reviewer: 2,
            },
            CompileRow04 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine04<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine04::<Draft>::REVIEW);
    }
}

mod flow05 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload05 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState05 {
        Draft,
        Review(ReviewPayload05),
        Published,
    }

    #[machine]
    pub struct CompileMachine05<CompileState05> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine05<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine05<Review> {
            self.transition_with(ReviewPayload05 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine05<Review> {
        fn publish(self) -> CompileMachine05<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow05 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine05)]
    impl CompileRow05 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload05> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload05 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine05::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow05 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow05 {
                status: 0,
                reviewer: 1,
            },
            CompileRow05 {
                status: 1,
                reviewer: 2,
            },
            CompileRow05 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine05<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine05::<Draft>::REVIEW);
    }
}

mod flow06 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload06 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState06 {
        Draft,
        Review(ReviewPayload06),
        Published,
    }

    #[machine]
    pub struct CompileMachine06<CompileState06> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine06<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine06<Review> {
            self.transition_with(ReviewPayload06 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine06<Review> {
        fn publish(self) -> CompileMachine06<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow06 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine06)]
    impl CompileRow06 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload06> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload06 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine06::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow06 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow06 {
                status: 0,
                reviewer: 1,
            },
            CompileRow06 {
                status: 1,
                reviewer: 2,
            },
            CompileRow06 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine06<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine06::<Draft>::REVIEW);
    }
}

mod flow07 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload07 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState07 {
        Draft,
        Review(ReviewPayload07),
        Published,
    }

    #[machine]
    pub struct CompileMachine07<CompileState07> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine07<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine07<Review> {
            self.transition_with(ReviewPayload07 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine07<Review> {
        fn publish(self) -> CompileMachine07<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow07 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine07)]
    impl CompileRow07 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload07> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload07 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine07::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow07 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow07 {
                status: 0,
                reviewer: 1,
            },
            CompileRow07 {
                status: 1,
                reviewer: 2,
            },
            CompileRow07 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine07<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine07::<Draft>::REVIEW);
    }
}

mod flow08 {
    use super::*;

    #[derive(Clone)]
    pub struct ReviewPayload08 {
        reviewer: u64,
    }

    #[state]
    pub enum CompileState08 {
        Draft,
        Review(ReviewPayload08),
        Published,
    }

    #[machine]
    pub struct CompileMachine08<CompileState08> {
        tenant: u64,
        shard: u64,
    }

    #[transition]
    impl CompileMachine08<Draft> {
        fn review(self, reviewer: u64) -> CompileMachine08<Review> {
            self.transition_with(ReviewPayload08 { reviewer })
        }
    }

    #[transition]
    impl CompileMachine08<Review> {
        fn publish(self) -> CompileMachine08<Published> {
            self.transition()
        }
    }

    #[derive(Clone)]
    pub struct CompileRow08 {
        status: u8,
        reviewer: u64,
    }

    #[validators(CompileMachine08)]
    impl CompileRow08 {
        fn is_draft(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 0 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_review(&self) -> statum::Result<ReviewPayload08> {
            let _ = tenant;
            let _ = shard;
            if self.status == 1 {
                Ok(ReviewPayload08 {
                    reviewer: self.reviewer,
                })
            } else {
                Err(statum::Error::InvalidState)
            }
        }

        fn is_published(&self) -> statum::Result<()> {
            let _ = tenant;
            let _ = shard;
            if self.status == 2 {
                Ok(())
            } else {
                Err(statum::Error::InvalidState)
            }
        }
    }

    pub fn exercise() {
        let machine = CompileMachine08::<Draft>::builder().tenant(1).shard(2).build();
        let machine = machine.review(9).publish();
        let _ = machine.tenant;

        let row = CompileRow08 {
            status: 1,
            reviewer: 7,
        };
        let _ = row.clone().into_machine().tenant(1).shard(2).build();
        let _ = row.into_machine().tenant(1).shard(2).build_report();

        let rows = vec![
            CompileRow08 {
                status: 0,
                reviewer: 1,
            },
            CompileRow08 {
                status: 1,
                reviewer: 2,
            },
            CompileRow08 {
                status: 2,
                reviewer: 3,
            },
        ];
        let _ = rows.into_machines().tenant(1).shard(2).build();

        let graph = <CompileMachine08<Draft> as MachineIntrospection>::GRAPH;
        let _ = graph.legal_targets(CompileMachine08::<Draft>::REVIEW);
    }
}

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
