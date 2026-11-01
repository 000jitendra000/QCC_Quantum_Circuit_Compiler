//! Compiler pass framework, pass manager, analysis invalidation, and execution pipelines.

pub mod context;
pub mod errors;
pub mod pass;
pub mod pass_manager;
pub mod statistics;

pub use context::{AnalysisId, PassContext, PreservedAnalyses};
pub use errors::{PassError, PassManagerError};
pub use pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
pub use pass_manager::PassManager;
pub use statistics::{
    PassExecutionStatus, PassExecutionTrace, PassManagerReport, PassStatistics, PassTraceStep,
};
