//! Integration test runner for the QCC compiler optimizer subsystem.

#[path = "optimizer/cancellation.rs"]
mod cancellation;
#[path = "optimizer/commutation.rs"]
mod commutation;
#[path = "optimizer/fusion.rs"]
mod fusion;
#[path = "optimizer/identity.rs"]
mod identity;
#[path = "optimizer/rotation_merge.rs"]
mod rotation_merge;
