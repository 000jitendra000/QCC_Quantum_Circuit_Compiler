//! Integration tests for synthesis templates and decomposition registry.

#[path = "synthesis/templates/swap.rs"]
mod swap;

#[path = "synthesis/templates/cx_cz.rs"]
mod cx_cz;

#[path = "synthesis/templates/toffoli.rs"]
mod toffoli;

#[path = "synthesis/templates/registry.rs"]
mod registry;
