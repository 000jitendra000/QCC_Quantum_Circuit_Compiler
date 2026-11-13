//! Top-level integration test entry point for QCC Routing module.

#[path = "routing/layout.rs"]
mod layout;

#[path = "routing/placement.rs"]
mod placement;

#[path = "routing/greedy.rs"]
mod greedy;

#[path = "routing/swap_rewrite.rs"]
mod swap_rewrite;

#[path = "routing/metrics.rs"]
mod metrics;

#[path = "routing/sabre_frontier.rs"]
mod sabre_frontier;

#[path = "routing/sabre_heuristic.rs"]
mod sabre_heuristic;

#[path = "routing/sabre_bidirectional.rs"]
mod sabre_bidirectional;

#[path = "routing/property.rs"]
mod property;

#[path = "routing/differential.rs"]
mod differential;

#[path = "routing/search_config.rs"]
mod search_config;
