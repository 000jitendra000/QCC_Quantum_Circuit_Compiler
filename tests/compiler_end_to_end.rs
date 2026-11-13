//! Cargo integration test target for the integrated compilation pipeline (Phase 59).

#[path = "compiler/end_to_end.rs"]
mod end_to_end;

#[path = "compiler/manifest.rs"]
mod manifest;

#[path = "compiler/pass_diagnostic.rs"]
mod pass_diagnostic;

#[path = "compiler/pass_instrumentation.rs"]
mod pass_instrumentation;

#[path = "compiler/qft_showcase.rs"]
mod qft_showcase;

#[path = "compiler/acceptance.rs"]
mod acceptance;
