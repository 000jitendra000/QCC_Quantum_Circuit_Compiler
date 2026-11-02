//! Compiler optimizer subsystem, gate algebra, passes, and identity registry.

pub mod algebra;
pub mod passes;

pub use algebra::*;
pub use passes::*;
