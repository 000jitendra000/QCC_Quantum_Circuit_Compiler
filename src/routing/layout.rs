//! Multi-stage circuit layout tracking representing initial, current, and final qubit assignments.
//!
//! Preserves both initial layout ($L_{\text{initial}}$ at $t=0$) and final layout ($L_{\text{final}}$
//! after all routing SWAP mutations) as required by project invariants and ADR-0001.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::routing::layout_map::{LayoutError, LayoutMap};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// Encapsulates the complete layout state across a circuit's compilation and routing lifecycle.
///
/// Explicitly maintains:
/// - `initial`: The physical qubit assignment at circuit entry ($L_{\text{initial}}$).
/// - `current`: The actively mutating layout during pass execution ($L_{\text{current}}$).
/// - `final_layout`: The physical qubit assignment at circuit exit ($L_{\text{final}}$).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawCircuitLayout", into = "RawCircuitLayout")]
pub struct CircuitLayout {
    initial: LayoutMap,
    current: LayoutMap,
    final_layout: LayoutMap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawCircuitLayout {
    initial: LayoutMap,
    current: LayoutMap,
    final_layout: LayoutMap,
}

impl From<CircuitLayout> for RawCircuitLayout {
    fn from(cl: CircuitLayout) -> Self {
        Self {
            initial: cl.initial,
            current: cl.current,
            final_layout: cl.final_layout,
        }
    }
}

impl TryFrom<RawCircuitLayout> for CircuitLayout {
    type Error = String;

    fn try_from(raw: RawCircuitLayout) -> Result<Self, Self::Error> {
        let cl = Self::with_layouts(raw.initial, raw.current, raw.final_layout)
            .map_err(|e| format!("deserialized CircuitLayout validation failed: {e}"))?;
        Ok(cl)
    }
}

impl CircuitLayout {
    /// Creates a new `CircuitLayout` initialized from an initial layout.
    ///
    /// Initially, `current` and `final_layout` are clones of `initial`.
    pub fn new(initial: LayoutMap) -> Self {
        let current = initial.clone();
        let final_layout = initial.clone();
        Self {
            initial,
            current,
            final_layout,
        }
    }

    /// Constructs a `CircuitLayout` from explicitly specified initial, current, and final layouts.
    ///
    /// Validates that:
    /// - Capacities (`num_logical` and `num_physical`) match across all three layouts.
    /// - The set of assigned logical qubits matches across all three layouts.
    /// - Each layout is internally valid.
    pub fn with_layouts(
        initial: LayoutMap,
        current: LayoutMap,
        final_layout: LayoutMap,
    ) -> Result<Self, LayoutError> {
        initial.validate()?;
        current.validate()?;
        final_layout.validate()?;

        if initial.num_logical() != current.num_logical() {
            return Err(LayoutError::CapacityMismatch {
                expected: initial.num_logical(),
                actual: current.num_logical(),
            });
        }
        if initial.num_physical() != current.num_physical() {
            return Err(LayoutError::CapacityMismatch {
                expected: initial.num_physical(),
                actual: current.num_physical(),
            });
        }
        if initial.num_logical() != final_layout.num_logical() {
            return Err(LayoutError::CapacityMismatch {
                expected: initial.num_logical(),
                actual: final_layout.num_logical(),
            });
        }
        if initial.num_physical() != final_layout.num_physical() {
            return Err(LayoutError::CapacityMismatch {
                expected: initial.num_physical(),
                actual: final_layout.num_physical(),
            });
        }

        if initial.assigned_logical_qubits() != current.assigned_logical_qubits() {
            return Err(LayoutError::InvalidState(
                "assigned logical qubit set differs between initial and current layouts"
                    .to_string(),
            ));
        }
        if initial.assigned_logical_qubits() != final_layout.assigned_logical_qubits() {
            return Err(LayoutError::InvalidState(
                "assigned logical qubit set differs between initial and final layouts".to_string(),
            ));
        }

        Ok(Self {
            initial,
            current,
            final_layout,
        })
    }

    /// Creates an identity `CircuitLayout` for given logical and physical capacities.
    pub fn identity(num_logical: usize, num_physical: usize) -> Result<Self, LayoutError> {
        let initial = LayoutMap::identity(num_logical, num_physical)?;
        Ok(Self::new(initial))
    }

    /// Creates an identity `CircuitLayout` for a given logical qubit count and hardware [`Target`].
    pub fn identity_for_target(num_logical: usize, target: &Target) -> Result<Self, LayoutError> {
        let initial = LayoutMap::identity_for_target(num_logical, target)?;
        Ok(Self::new(initial))
    }

    /// Creates an identity `CircuitLayout` derived from a logical [`Circuit`] and hardware [`Target`].
    pub fn identity_for_circuit(circuit: &Circuit, target: &Target) -> Result<Self, LayoutError> {
        let num_logical = circuit.num_qubits();
        Self::identity_for_target(num_logical, target)
    }

    /// Returns a reference to the initial layout ($L_{\text{initial}}$).
    #[inline]
    pub fn initial(&self) -> &LayoutMap {
        &self.initial
    }

    /// Returns a reference to the active/current layout ($L_{\text{current}}$).
    #[inline]
    pub fn current(&self) -> &LayoutMap {
        &self.current
    }

    /// Returns a mutable reference to the active/current layout ($L_{\text{current}}$).
    #[inline]
    pub fn current_mut(&mut self) -> &mut LayoutMap {
        &mut self.current
    }

    /// Returns a reference to the final layout ($L_{\text{final}}$).
    #[inline]
    pub fn final_layout(&self) -> &LayoutMap {
        &self.final_layout
    }

    /// Applies a physical `SWAP(p1, p2)` gate mutation to the current layout and updates `final_layout`.
    ///
    /// Every router insertion of a SWAP gate must invoke this method to maintain layout invariants.
    pub fn apply_physical_swap(
        &mut self,
        p1: PhysicalQubitId,
        p2: PhysicalQubitId,
    ) -> Result<(), LayoutError> {
        self.current.swap_physical(p1, p2)?;
        self.final_layout = self.current.clone();
        Ok(())
    }

    /// Commits the active current layout to `final_layout`.
    #[inline]
    pub fn commit_final(&mut self) {
        self.final_layout = self.current.clone();
    }

    /// Validates all three layouts and checks consistency between them.
    pub fn validate(&self) -> Result<(), LayoutError> {
        self.initial.validate()?;
        self.current.validate()?;
        self.final_layout.validate()?;

        if self.initial.num_logical() != self.current.num_logical()
            || self.initial.num_physical() != self.current.num_physical()
        {
            return Err(LayoutError::CapacityMismatch {
                expected: self.initial.num_logical(),
                actual: self.current.num_logical(),
            });
        }

        if self.initial.num_logical() != self.final_layout.num_logical()
            || self.initial.num_physical() != self.final_layout.num_physical()
        {
            return Err(LayoutError::CapacityMismatch {
                expected: self.initial.num_logical(),
                actual: self.final_layout.num_logical(),
            });
        }

        if self.initial.assigned_logical_qubits() != self.current.assigned_logical_qubits() {
            return Err(LayoutError::InvalidState(
                "assigned logical qubit set mismatch between initial and current layouts"
                    .to_string(),
            ));
        }

        if self.initial.assigned_logical_qubits() != self.final_layout.assigned_logical_qubits() {
            return Err(LayoutError::InvalidState(
                "assigned logical qubit set mismatch between initial and final layouts".to_string(),
            ));
        }

        Ok(())
    }

    /// Serializes `CircuitLayout` to a deterministic JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserializes and validates a `CircuitLayout` from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, LayoutError> {
        serde_json::from_str(json).map_err(|e| LayoutError::DeserializationError(e.to_string()))
    }
}

impl fmt::Display for CircuitLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CircuitLayout(\n  Initial: {}\n  Current: {}\n  Final:   {}\n)",
            self.initial, self.current, self.final_layout
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::wire::QubitId;

    #[test]
    fn test_circuit_layout_lifecycle() {
        let mut cl = CircuitLayout::identity(2, 4).unwrap();
        assert_eq!(
            cl.initial().get_physical(QubitId(0)),
            Some(PhysicalQubitId(0))
        );
        assert_eq!(
            cl.initial().get_physical(QubitId(1)),
            Some(PhysicalQubitId(1))
        );
        assert_eq!(
            cl.final_layout().get_physical(QubitId(0)),
            Some(PhysicalQubitId(0))
        );

        // Apply physical SWAP(0, 2)
        cl.apply_physical_swap(PhysicalQubitId(0), PhysicalQubitId(2))
            .unwrap();

        // Initial must remain unchanged!
        assert_eq!(
            cl.initial().get_physical(QubitId(0)),
            Some(PhysicalQubitId(0))
        );
        // Current and Final must reflect the SWAP
        assert_eq!(
            cl.current().get_physical(QubitId(0)),
            Some(PhysicalQubitId(2))
        );
        assert_eq!(
            cl.final_layout().get_physical(QubitId(0)),
            Some(PhysicalQubitId(2))
        );
        assert_eq!(
            cl.current().get_physical(QubitId(1)),
            Some(PhysicalQubitId(1))
        );

        assert!(cl.validate().is_ok());
    }

    #[test]
    fn test_serde_circuit_layout_roundtrip() {
        let mut cl = CircuitLayout::identity(2, 3).unwrap();
        cl.apply_physical_swap(PhysicalQubitId(0), PhysicalQubitId(1))
            .unwrap();

        let json = cl.to_json().unwrap();
        let recovered = CircuitLayout::from_json(&json).unwrap();
        assert_eq!(cl, recovered);
    }
}
