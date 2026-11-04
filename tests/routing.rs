//! Top-level integration test entry point for QCC Routing module.

#[path = "routing/layout.rs"]
mod layout;

#[path = "routing/placement.rs"]
mod placement;

#[path = "routing/greedy.rs"]
mod greedy;

#[path = "routing/swap_rewrite.rs"]
mod swap_rewrite;
