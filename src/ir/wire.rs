//! Wire identifiers and per-wire access state for hazard tracking.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::node::OperationId;

/// Strongly typed identifier for a logical or physical qubit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QubitId(pub usize);

impl fmt::Display for QubitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "q[{}]", self.0)
    }
}

/// Strongly typed identifier for a classical bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClassicalBitId(pub usize);

impl fmt::Display for ClassicalBitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c[{}]", self.0)
    }
}

/// Disjoint identifier for quantum and classical wires in QccIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WireId {
    /// A quantum wire carrying state evolution of a qubit.
    Quantum(QubitId),
    /// A classical wire carrying measurement bits or classical control signals.
    Classical(ClassicalBitId),
}

impl fmt::Display for WireId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quantum(q) => write!(f, "{q}"),
            Self::Classical(c) => write!(f, "{c}"),
        }
    }
}

/// Per-wire access state maintained to accurately track RAW, WAR, and WAW data hazards.
///
/// A single last-operation pointer is strictly insufficient for classical wires with
/// multiple outstanding readers before a subsequent write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WireAccessState {
    /// Most recent operation that wrote or mutated this wire.
    pub last_writer: Option<OperationId>,
    /// All operations that read this wire since the `last_writer`.
    pub outstanding_readers: Vec<OperationId>,
}

impl WireAccessState {
    /// Creates a new empty `WireAccessState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a read access by `reader`.
    ///
    /// Returns the `last_writer` that this read induces a Read-After-Write (RAW) dependency on.
    pub fn record_read(&mut self, reader: OperationId) -> Option<OperationId> {
        if !self.outstanding_readers.contains(&reader) {
            self.outstanding_readers.push(reader);
        }
        self.last_writer
    }

    /// Records a write access by `writer`.
    ///
    /// Returns:
    /// - `Option<OperationId>`: Prior writer for Write-After-Write (WAW) dependency.
    /// - `Vec<OperationId>`: All outstanding readers for Write-After-Read (WAR) dependencies.
    ///
    /// Clears the outstanding reader set and updates `last_writer` to `writer`.
    pub fn record_write(&mut self, writer: OperationId) -> (Option<OperationId>, Vec<OperationId>) {
        let prev_writer = self.last_writer;
        let prev_readers = std::mem::take(&mut self.outstanding_readers);
        self.last_writer = Some(writer);
        (prev_writer, prev_readers)
    }

    /// Records an in-place quantum state transformation (both reads previous state and writes new state).
    ///
    /// Induces dependency on `prev_writer` and all `prev_readers`, then sets `writer` as new writer.
    pub fn record_quantum_update(
        &mut self,
        op_id: OperationId,
    ) -> (Option<OperationId>, Vec<OperationId>) {
        self.record_write(op_id)
    }
}
