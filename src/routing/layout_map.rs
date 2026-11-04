//! Injective mapping between logical program qubits and physical target device qubits.
//!
//! Enforces strict separation between logical [`QubitId`] and physical [`PhysicalQubitId`],
//! bidirectional consistency, injectivity, and explicit tracking of unassigned or idle
//! physical qubits in compliance with ADR-0001.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::wire::QubitId;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// Type alias emphasizing logical qubit identity.
pub type LogicalQubitId = QubitId;

/// Errors that can occur during layout construction, mutation, or validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutError {
    /// Logical qubit index is out of bounds for the layout capacity.
    LogicalQubitOutOfBounds {
        /// Offending logical qubit.
        qubit: QubitId,
        /// Maximum logical capacity.
        capacity: usize,
    },
    /// Physical qubit index is out of bounds for the target capacity.
    PhysicalQubitOutOfBounds {
        /// Offending physical qubit.
        qubit: PhysicalQubitId,
        /// Maximum physical capacity.
        capacity: usize,
    },
    /// Physical qubit was assigned to more than one logical qubit (violates injectivity).
    DuplicatePhysicalAssignment {
        /// Physical qubit assigned multiple times.
        physical: PhysicalQubitId,
        /// Existing logical qubit holding this physical qubit.
        existing_logical: QubitId,
        /// Attempted new logical qubit.
        new_logical: QubitId,
    },
    /// Logical qubit was assigned to more than one physical qubit.
    DuplicateLogicalAssignment {
        /// Logical qubit with multiple assignments.
        logical: QubitId,
        /// Existing physical qubit assigned to this logical qubit.
        existing_physical: PhysicalQubitId,
        /// Attempted new physical qubit.
        new_physical: PhysicalQubitId,
    },
    /// Forward mapping points to physical qubit, but reverse mapping does not match.
    InconsistentReverseMapping {
        /// Logical qubit queried.
        logical: QubitId,
        /// Physical qubit in forward map.
        forward_physical: PhysicalQubitId,
        /// Logical qubit found in reverse map for that physical qubit.
        reverse_logical: Option<QubitId>,
    },
    /// Reverse mapping points to logical qubit, but forward mapping does not match.
    InconsistentForwardMapping {
        /// Physical qubit queried.
        physical: PhysicalQubitId,
        /// Logical qubit in reverse map.
        reverse_logical: QubitId,
        /// Physical qubit found in forward map for that logical qubit.
        forward_physical: Option<PhysicalQubitId>,
    },
    /// Number of logical qubits exceeds physical target device capacity.
    CapacityExceeded {
        /// Number of logical qubits required.
        required: usize,
        /// Available physical qubits on device.
        capacity: usize,
    },
    /// Capacity mismatch between layouts or targets.
    CapacityMismatch {
        /// Expected capacity.
        expected: usize,
        /// Actual capacity found.
        actual: usize,
    },
    /// Invalid permutation sequence provided.
    InvalidPermutation(String),
    /// Deserialization validation error.
    DeserializationError(String),
    /// Inconsistent internal or lifecycle state.
    InvalidState(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LogicalQubitOutOfBounds { qubit, capacity } => {
                write!(
                    f,
                    "logical qubit {qubit} is out of bounds for logical capacity {capacity}"
                )
            }
            Self::PhysicalQubitOutOfBounds { qubit, capacity } => {
                write!(
                    f,
                    "physical qubit {qubit} is out of bounds for physical capacity {capacity}"
                )
            }
            Self::DuplicatePhysicalAssignment {
                physical,
                existing_logical,
                new_logical,
            } => {
                write!(
                    f,
                    "physical qubit {physical} is already occupied by logical {existing_logical}, cannot assign to {new_logical}"
                )
            }
            Self::DuplicateLogicalAssignment {
                logical,
                existing_physical,
                new_physical,
            } => {
                write!(
                    f,
                    "logical qubit {logical} is already mapped to physical {existing_physical}, cannot reassign to {new_physical}"
                )
            }
            Self::InconsistentReverseMapping {
                logical,
                forward_physical,
                reverse_logical,
            } => {
                write!(
                    f,
                    "layout inconsistency: {logical} -> {forward_physical}, but reverse map has {forward_physical} -> {reverse_logical:?}"
                )
            }
            Self::InconsistentForwardMapping {
                physical,
                reverse_logical,
                forward_physical,
            } => {
                write!(
                    f,
                    "layout inconsistency: reverse {physical} -> {reverse_logical}, but forward map has {reverse_logical} -> {forward_physical:?}"
                )
            }
            Self::CapacityExceeded { required, capacity } => {
                write!(
                    f,
                    "logical qubit count ({required}) exceeds device physical qubit capacity ({capacity})"
                )
            }
            Self::CapacityMismatch { expected, actual } => {
                write!(
                    f,
                    "layout capacity mismatch: expected {expected}, found {actual}"
                )
            }
            Self::InvalidPermutation(msg) => write!(f, "invalid permutation: {msg}"),
            Self::DeserializationError(msg) => write!(f, "layout deserialization error: {msg}"),
            Self::InvalidState(msg) => write!(f, "invalid layout state: {msg}"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Injective partial or total mapping between logical program qubits and physical target qubits.
///
/// Uses sorted [`BTreeMap`] structures internally to ensure strictly deterministic iteration order
/// and reproducible serialization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawLayoutMap", into = "RawLayoutMap")]
pub struct LayoutMap {
    num_logical: usize,
    num_physical: usize,
    logical_to_physical: BTreeMap<QubitId, PhysicalQubitId>,
    physical_to_logical: BTreeMap<PhysicalQubitId, QubitId>,
}

/// Raw representation used for validating serialization and deserialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawLayoutMap {
    num_logical: usize,
    num_physical: usize,
    mappings: Vec<(usize, u32)>,
}

impl From<LayoutMap> for RawLayoutMap {
    fn from(layout: LayoutMap) -> Self {
        let mappings = layout
            .logical_to_physical
            .iter()
            .map(|(q, p)| (q.0, p.0))
            .collect();
        Self {
            num_logical: layout.num_logical,
            num_physical: layout.num_physical,
            mappings,
        }
    }
}

impl TryFrom<RawLayoutMap> for LayoutMap {
    type Error = String;

    fn try_from(raw: RawLayoutMap) -> Result<Self, Self::Error> {
        let mut layout = Self::new(raw.num_logical, raw.num_physical);
        for (logical, physical) in raw.mappings {
            layout
                .assign(QubitId(logical), PhysicalQubitId(physical))
                .map_err(|e| format!("deserialized layout is invalid: {e}"))?;
        }
        layout
            .validate()
            .map_err(|e| format!("deserialized layout validation failed: {e}"))?;
        Ok(layout)
    }
}

impl LayoutMap {
    /// Creates an empty layout with the given logical and physical capacities.
    #[inline]
    pub fn new(num_logical: usize, num_physical: usize) -> Self {
        Self {
            num_logical,
            num_physical,
            logical_to_physical: BTreeMap::new(),
            physical_to_logical: BTreeMap::new(),
        }
    }

    /// Creates an identity layout mapping logical qubit `i` to physical qubit `i` for `i` in `0..num_logical`.
    ///
    /// Fails with [`LayoutError::CapacityExceeded`] if `num_logical > num_physical`.
    pub fn identity(num_logical: usize, num_physical: usize) -> Result<Self, LayoutError> {
        if num_logical > num_physical {
            return Err(LayoutError::CapacityExceeded {
                required: num_logical,
                capacity: num_physical,
            });
        }
        let mut layout = Self::new(num_logical, num_physical);
        for i in 0..num_logical {
            layout.assign(QubitId(i), PhysicalQubitId(i as u32))?;
        }
        Ok(layout)
    }

    /// Creates an identity layout sized to a specific [`Target`].
    pub fn identity_for_target(num_logical: usize, target: &Target) -> Result<Self, LayoutError> {
        Self::identity(num_logical, target.num_qubits)
    }

    /// Constructs a layout from an explicit list of `(logical, physical)` pairs.
    pub fn from_mappings(
        num_logical: usize,
        num_physical: usize,
        mappings: impl IntoIterator<Item = (QubitId, PhysicalQubitId)>,
    ) -> Result<Self, LayoutError> {
        let mut layout = Self::new(num_logical, num_physical);
        for (l, p) in mappings {
            layout.assign(l, p)?;
        }
        layout.validate()?;
        Ok(layout)
    }

    /// Constructs a layout mapping logical qubits `0..slice.len()` to the given physical qubits.
    pub fn from_slice(
        num_physical: usize,
        physical_qubits: &[PhysicalQubitId],
    ) -> Result<Self, LayoutError> {
        let num_logical = physical_qubits.len();
        let mut layout = Self::new(num_logical, num_physical);
        for (i, &p) in physical_qubits.iter().enumerate() {
            layout.assign(QubitId(i), p)?;
        }
        layout.validate()?;
        Ok(layout)
    }

    /// Constructs a bijective layout from a permutation vector where logical `i` maps to physical `perm[i]`.
    pub fn from_permutation(permutation: &[usize]) -> Result<Self, LayoutError> {
        let n = permutation.len();
        let mut layout = Self::new(n, n);
        for (i, &p) in permutation.iter().enumerate() {
            layout.assign(QubitId(i), PhysicalQubitId(p as u32))?;
        }
        layout.validate()?;
        Ok(layout)
    }

    /// Returns the declared logical qubit capacity.
    #[inline]
    pub fn num_logical(&self) -> usize {
        self.num_logical
    }

    /// Returns the declared physical device qubit capacity.
    #[inline]
    pub fn num_physical(&self) -> usize {
        self.num_physical
    }

    /// Returns the number of logical qubits currently assigned to physical qubits.
    #[inline]
    pub fn num_assigned(&self) -> usize {
        self.logical_to_physical.len()
    }

    /// Returns the number of physical qubits that are currently unused/idle.
    #[inline]
    pub fn num_unused_physical(&self) -> usize {
        self.num_physical.saturating_sub(self.num_assigned())
    }

    /// Returns true if all logical qubits (`0..num_logical`) have been assigned physical locations.
    #[inline]
    pub fn is_fully_assigned(&self) -> bool {
        self.num_assigned() == self.num_logical
    }

    /// Returns true if this is a bijection between all logical and all physical qubits.
    #[inline]
    pub fn is_bijective(&self) -> bool {
        self.num_logical == self.num_physical && self.num_assigned() == self.num_logical
    }

    /// Queries the physical qubit mapped to a logical qubit.
    #[inline]
    pub fn get_physical(&self, logical: QubitId) -> Option<PhysicalQubitId> {
        self.logical_to_physical.get(&logical).copied()
    }

    /// Queries the logical qubit mapped to a physical qubit.
    #[inline]
    pub fn get_logical(&self, physical: PhysicalQubitId) -> Option<QubitId> {
        self.physical_to_logical.get(&physical).copied()
    }

    /// Returns true if the logical qubit is mapped.
    #[inline]
    pub fn contains_logical(&self, logical: QubitId) -> bool {
        self.logical_to_physical.contains_key(&logical)
    }

    /// Returns true if the physical qubit is occupied by a logical qubit.
    #[inline]
    pub fn is_physical_used(&self, physical: PhysicalQubitId) -> bool {
        self.physical_to_logical.contains_key(&physical)
    }

    /// Returns true if the physical qubit is unused (idle ancilla per ADR-0001).
    #[inline]
    pub fn is_physical_idle(&self, physical: PhysicalQubitId) -> bool {
        (physical.0 as usize) < self.num_physical && !self.is_physical_used(physical)
    }

    /// Returns a sorted vector of all currently assigned logical qubits.
    pub fn assigned_logical_qubits(&self) -> Vec<QubitId> {
        self.logical_to_physical.keys().copied().collect()
    }

    /// Returns a sorted vector of all currently occupied physical qubits.
    pub fn assigned_physical_qubits(&self) -> Vec<PhysicalQubitId> {
        self.physical_to_logical.keys().copied().collect()
    }

    /// Returns a sorted vector of all unused physical qubits on the target device.
    ///
    /// Essential for ADR-0001 ancilla zero-state verification.
    pub fn unused_physical_qubits(&self) -> Vec<PhysicalQubitId> {
        (0..self.num_physical as u32)
            .map(PhysicalQubitId)
            .filter(|p| !self.is_physical_used(*p))
            .collect()
    }

    /// Returns a sorted vector of all unassigned logical qubits.
    pub fn unassigned_logical_qubits(&self) -> Vec<QubitId> {
        (0..self.num_logical)
            .map(QubitId)
            .filter(|q| !self.contains_logical(*q))
            .collect()
    }

    /// Assigns a logical qubit to a physical qubit.
    ///
    /// Fails if:
    /// - `logical.0 >= num_logical`
    /// - `physical.0 >= num_physical`
    /// - `logical` is already assigned to a physical qubit
    /// - `physical` is already occupied by another logical qubit
    pub fn assign(
        &mut self,
        logical: QubitId,
        physical: PhysicalQubitId,
    ) -> Result<(), LayoutError> {
        if logical.0 >= self.num_logical {
            return Err(LayoutError::LogicalQubitOutOfBounds {
                qubit: logical,
                capacity: self.num_logical,
            });
        }
        if physical.0 as usize >= self.num_physical {
            return Err(LayoutError::PhysicalQubitOutOfBounds {
                qubit: physical,
                capacity: self.num_physical,
            });
        }
        if let Some(&existing_p) = self.logical_to_physical.get(&logical) {
            return Err(LayoutError::DuplicateLogicalAssignment {
                logical,
                existing_physical: existing_p,
                new_physical: physical,
            });
        }
        if let Some(&existing_l) = self.physical_to_logical.get(&physical) {
            return Err(LayoutError::DuplicatePhysicalAssignment {
                physical,
                existing_logical: existing_l,
                new_logical: logical,
            });
        }

        self.logical_to_physical.insert(logical, physical);
        self.physical_to_logical.insert(physical, logical);
        Ok(())
    }

    /// Removes the assignment for a logical qubit, returning the freed physical qubit if present.
    pub fn unassign(&mut self, logical: QubitId) -> Option<PhysicalQubitId> {
        if let Some(p) = self.logical_to_physical.remove(&logical) {
            self.physical_to_logical.remove(&p);
            Some(p)
        } else {
            None
        }
    }

    /// Reassigns a logical qubit to a new physical location, freeing its prior physical qubit.
    ///
    /// Fails if the new physical qubit is already occupied by a *different* logical qubit,
    /// or if bounds checks fail.
    pub fn reassign(
        &mut self,
        logical: QubitId,
        new_physical: PhysicalQubitId,
    ) -> Result<Option<PhysicalQubitId>, LayoutError> {
        if logical.0 >= self.num_logical {
            return Err(LayoutError::LogicalQubitOutOfBounds {
                qubit: logical,
                capacity: self.num_logical,
            });
        }
        if new_physical.0 as usize >= self.num_physical {
            return Err(LayoutError::PhysicalQubitOutOfBounds {
                qubit: new_physical,
                capacity: self.num_physical,
            });
        }
        if let Some(&occupant) = self.physical_to_logical.get(&new_physical) {
            if occupant != logical {
                return Err(LayoutError::DuplicatePhysicalAssignment {
                    physical: new_physical,
                    existing_logical: occupant,
                    new_logical: logical,
                });
            }
        }

        let old_p = self.unassign(logical);
        self.assign(logical, new_physical)?;
        Ok(old_p)
    }

    /// Swaps the logical occupants of two physical qubits.
    ///
    /// This is the fundamental layout mutation induced by a routing `SWAP(p1, p2)` gate:
    /// - If both `p1` and `p2` hold logical qubits, their assignments are interchanged.
    /// - If `p1` holds logical `q` and `p2` is idle, `q` moves to `p2` and `p1` becomes idle.
    /// - If `p2` holds logical `q` and `p1` is idle, `q` moves to `p1` and `p2` becomes idle.
    /// - If both are idle, this is a valid no-op.
    pub fn swap_physical(
        &mut self,
        p1: PhysicalQubitId,
        p2: PhysicalQubitId,
    ) -> Result<(), LayoutError> {
        if p1.0 as usize >= self.num_physical {
            return Err(LayoutError::PhysicalQubitOutOfBounds {
                qubit: p1,
                capacity: self.num_physical,
            });
        }
        if p2.0 as usize >= self.num_physical {
            return Err(LayoutError::PhysicalQubitOutOfBounds {
                qubit: p2,
                capacity: self.num_physical,
            });
        }
        if p1 == p2 {
            return Ok(());
        }

        let l1 = self.physical_to_logical.remove(&p1);
        let l2 = self.physical_to_logical.remove(&p2);

        if let Some(q1) = l1 {
            self.logical_to_physical.insert(q1, p2);
            self.physical_to_logical.insert(p2, q1);
        } else {
            self.physical_to_logical.remove(&p2);
        }

        if let Some(q2) = l2 {
            self.logical_to_physical.insert(q2, p1);
            self.physical_to_logical.insert(p1, q2);
        } else {
            self.physical_to_logical.remove(&p1);
        }

        Ok(())
    }

    /// Swaps the physical positions of two logical qubits.
    ///
    /// Fails if either logical qubit is unassigned or out of bounds.
    pub fn swap_logical(&mut self, q1: QubitId, q2: QubitId) -> Result<(), LayoutError> {
        if q1.0 >= self.num_logical {
            return Err(LayoutError::LogicalQubitOutOfBounds {
                qubit: q1,
                capacity: self.num_logical,
            });
        }
        if q2.0 >= self.num_logical {
            return Err(LayoutError::LogicalQubitOutOfBounds {
                qubit: q2,
                capacity: self.num_logical,
            });
        }
        if q1 == q2 {
            return Ok(());
        }

        let p1 = self
            .get_physical(q1)
            .ok_or(LayoutError::InvalidState(format!(
                "logical qubit {q1} is unassigned"
            )))?;
        let p2 = self
            .get_physical(q2)
            .ok_or(LayoutError::InvalidState(format!(
                "logical qubit {q2} is unassigned"
            )))?;

        self.logical_to_physical.insert(q1, p2);
        self.logical_to_physical.insert(q2, p1);
        self.physical_to_logical.insert(p1, q2);
        self.physical_to_logical.insert(p2, q1);

        Ok(())
    }

    /// Validates all internal invariants:
    /// 1. Bounds: all mapped logical qubits `< num_logical`, all mapped physical qubits `< num_physical`.
    /// 2. Injectivity: size of forward map == size of reverse map.
    /// 3. Bidirectional consistency: `forward[q] == p <=> reverse[p] == q`.
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.logical_to_physical.len() != self.physical_to_logical.len() {
            return Err(LayoutError::InvalidState(format!(
                "map sizes differ: forward has {} entries, reverse has {} entries",
                self.logical_to_physical.len(),
                self.physical_to_logical.len()
            )));
        }

        for (&q, &p) in &self.logical_to_physical {
            if q.0 >= self.num_logical {
                return Err(LayoutError::LogicalQubitOutOfBounds {
                    qubit: q,
                    capacity: self.num_logical,
                });
            }
            if p.0 as usize >= self.num_physical {
                return Err(LayoutError::PhysicalQubitOutOfBounds {
                    qubit: p,
                    capacity: self.num_physical,
                });
            }
            let rev = self.physical_to_logical.get(&p).copied();
            if rev != Some(q) {
                return Err(LayoutError::InconsistentReverseMapping {
                    logical: q,
                    forward_physical: p,
                    reverse_logical: rev,
                });
            }
        }

        for (&p, &q) in &self.physical_to_logical {
            let fwd = self.logical_to_physical.get(&q).copied();
            if fwd != Some(p) {
                return Err(LayoutError::InconsistentForwardMapping {
                    physical: p,
                    reverse_logical: q,
                    forward_physical: fwd,
                });
            }
        }

        Ok(())
    }

    /// Serializes the layout to a deterministic JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserializes and validates a layout from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, LayoutError> {
        serde_json::from_str(json).map_err(|e| LayoutError::DeserializationError(e.to_string()))
    }
}

/// Convenience alias for [`LayoutMap`].
pub type Layout = LayoutMap;

impl fmt::Display for LayoutMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Layout(L:{}/P:{} [", self.num_logical, self.num_physical)?;
        let mut first = true;
        for (q, p) in &self.logical_to_physical {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{q}->{p}")?;
            first = false;
        }
        let unused = self.unused_physical_qubits();
        if !unused.is_empty() {
            write!(f, " | idle:")?;
            for p in unused {
                write!(f, " {p}")?;
            }
        }
        write!(f, "])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_layout() {
        let layout = Layout::identity(3, 5).unwrap();
        assert_eq!(layout.num_logical(), 3);
        assert_eq!(layout.num_physical(), 5);
        assert_eq!(layout.num_assigned(), 3);
        assert_eq!(layout.num_unused_physical(), 2);
        assert_eq!(layout.get_physical(QubitId(0)), Some(PhysicalQubitId(0)));
        assert_eq!(layout.get_physical(QubitId(1)), Some(PhysicalQubitId(1)));
        assert_eq!(layout.get_physical(QubitId(2)), Some(PhysicalQubitId(2)));
        assert_eq!(layout.get_physical(QubitId(3)), None);
        assert_eq!(layout.get_logical(PhysicalQubitId(0)), Some(QubitId(0)));
        assert_eq!(layout.get_logical(PhysicalQubitId(3)), None);
        assert!(layout.is_physical_idle(PhysicalQubitId(3)));
        assert!(layout.is_physical_idle(PhysicalQubitId(4)));
        assert!(!layout.is_physical_idle(PhysicalQubitId(0)));
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn test_capacity_exceeded() {
        let err = Layout::identity(5, 3).unwrap_err();
        match err {
            LayoutError::CapacityExceeded { required, capacity } => {
                assert_eq!(required, 5);
                assert_eq!(capacity, 3);
            }
            other => panic!("expected CapacityExceeded, got {other:?}"),
        }
    }

    #[test]
    fn test_swap_physical() {
        let mut layout = Layout::identity(2, 4).unwrap();
        // q[0]->p[0], q[1]->p[1], p[2] idle, p[3] idle
        layout
            .swap_physical(PhysicalQubitId(0), PhysicalQubitId(1))
            .unwrap();
        assert_eq!(layout.get_physical(QubitId(0)), Some(PhysicalQubitId(1)));
        assert_eq!(layout.get_physical(QubitId(1)), Some(PhysicalQubitId(0)));

        // Swap occupied p[1] with idle p[2]
        layout
            .swap_physical(PhysicalQubitId(1), PhysicalQubitId(2))
            .unwrap();
        assert_eq!(layout.get_physical(QubitId(0)), Some(PhysicalQubitId(2)));
        assert_eq!(layout.get_logical(PhysicalQubitId(1)), None);
        assert!(layout.is_physical_idle(PhysicalQubitId(1)));

        assert!(layout.validate().is_ok());
    }

    #[test]
    fn test_duplicate_physical_rejection() {
        let mut layout = Layout::new(3, 3);
        layout.assign(QubitId(0), PhysicalQubitId(1)).unwrap();
        let err = layout.assign(QubitId(1), PhysicalQubitId(1)).unwrap_err();
        match err {
            LayoutError::DuplicatePhysicalAssignment { physical, .. } => {
                assert_eq!(physical, PhysicalQubitId(1));
            }
            other => panic!("expected DuplicatePhysicalAssignment, got {other:?}"),
        }
    }

    #[test]
    fn test_serde_validation_roundtrip() {
        let layout = Layout::from_mappings(
            2,
            4,
            [
                (QubitId(0), PhysicalQubitId(3)),
                (QubitId(1), PhysicalQubitId(1)),
            ],
        )
        .unwrap();

        let json = layout.to_json().unwrap();
        let recovered = Layout::from_json(&json).unwrap();
        assert_eq!(layout, recovered);

        // Corrupt JSON with duplicate physical assignment
        let corrupt = r#"{"num_logical":2,"num_physical":4,"mappings":[[0,1],[1,1]]}"#;
        let err = Layout::from_json(corrupt).unwrap_err();
        match err {
            LayoutError::DeserializationError(msg) => {
                assert!(msg.contains("duplicate") || msg.contains("occupied"));
            }
            other => panic!("expected DeserializationError, got {other:?}"),
        }
    }
}
