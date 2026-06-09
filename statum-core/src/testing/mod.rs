//! Test helpers for asserting generated protocol metadata.
//!
//! These helpers observe [`MachineGraph`](crate::MachineGraph) metadata emitted
//! for the active build. They do not prove that transition method bodies ran or
//! that persisted state is complete.

pub mod graph;
pub mod rehydrate;
pub mod walks;
