//! Strongly typed qubit, classical bit, and register handles.

use std::fmt;
use std::ops::Index;

use crate::builder::errors::BuilderError;
use crate::ir::wire::{ClassicalBitId, QubitId};

/// Strongly typed, `Copy` handle to a logical qubit managed by a specific [`CircuitBuilder`](crate::builder::CircuitBuilder).
///
/// Guaranteed to carry its originating builder ID to prevent accidental cross-builder leakage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QubitHandle {
    /// ID of the originating `CircuitBuilder` instance.
    pub(crate) builder_id: u64,
    /// Global flat [`QubitId`] in the circuit.
    pub(crate) qubit_id: QubitId,
}

impl QubitHandle {
    /// Creates a new `QubitHandle`.
    #[inline]
    pub(crate) fn new(builder_id: u64, qubit_id: QubitId) -> Self {
        Self {
            builder_id,
            qubit_id,
        }
    }

    /// Returns the global flat [`QubitId`] corresponding to this handle.
    #[inline]
    pub fn qubit_id(&self) -> QubitId {
        self.qubit_id
    }

    /// Returns the raw numeric index of this qubit in the circuit's flat qubit index space.
    #[inline]
    pub fn index(&self) -> usize {
        self.qubit_id.0
    }

    /// Returns the ID of the builder that created this handle.
    #[inline]
    pub fn builder_id(&self) -> u64 {
        self.builder_id
    }
}

impl fmt::Display for QubitHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.qubit_id)
    }
}

/// Strongly typed, `Copy` handle to a classical bit managed by a specific [`CircuitBuilder`](crate::builder::CircuitBuilder).
///
/// Guaranteed to carry its originating builder ID to prevent accidental cross-builder leakage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassicalBitHandle {
    /// ID of the originating `CircuitBuilder` instance.
    pub(crate) builder_id: u64,
    /// Global flat [`ClassicalBitId`] in the circuit.
    pub(crate) bit_id: ClassicalBitId,
}

impl ClassicalBitHandle {
    /// Creates a new `ClassicalBitHandle`.
    #[inline]
    pub(crate) fn new(builder_id: u64, bit_id: ClassicalBitId) -> Self {
        Self { builder_id, bit_id }
    }

    /// Returns the global flat [`ClassicalBitId`] corresponding to this handle.
    #[inline]
    pub fn bit_id(&self) -> ClassicalBitId {
        self.bit_id
    }

    /// Returns the raw numeric index of this bit in the circuit's flat classical bit index space.
    #[inline]
    pub fn index(&self) -> usize {
        self.bit_id.0
    }

    /// Returns the ID of the builder that created this handle.
    #[inline]
    pub fn builder_id(&self) -> u64 {
        self.builder_id
    }
}

impl fmt::Display for ClassicalBitHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.bit_id)
    }
}

/// Handle to a declared quantum register, providing safe indexing and bounds-checked access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantumRegisterHandle {
    pub(crate) builder_id: u64,
    pub(crate) name: String,
    pub(crate) start_qubit: usize,
    pub(crate) size: usize,
    pub(crate) handles: Vec<QubitHandle>,
}

impl QuantumRegisterHandle {
    /// Creates a new `QuantumRegisterHandle`.
    pub(crate) fn new(builder_id: u64, name: String, start_qubit: usize, size: usize) -> Self {
        let mut handles = Vec::with_capacity(size);
        for i in 0..size {
            handles.push(QubitHandle::new(builder_id, QubitId(start_qubit + i)));
        }
        Self {
            builder_id,
            name,
            start_qubit,
            size,
            handles,
        }
    }

    /// Returns the name of this quantum register.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of qubits in this register.
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns `true` if this register has size zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the base flat qubit index in the circuit.
    #[inline]
    pub fn start_qubit(&self) -> usize {
        self.start_qubit
    }

    /// Returns the originating builder ID.
    #[inline]
    pub fn builder_id(&self) -> u64 {
        self.builder_id
    }

    /// Returns a validated [`QubitHandle`] for the qubit at `index` in this register.
    ///
    /// # Errors
    /// Returns [`BuilderError::IndexOutOfBounds`] if `index >= self.size()`.
    pub fn q(&self, index: usize) -> Result<QubitHandle, BuilderError> {
        if index < self.size {
            Ok(self.handles[index])
        } else {
            Err(BuilderError::IndexOutOfBounds {
                register: self.name.clone(),
                index,
                size: self.size,
            })
        }
    }

    /// Convenient alias for [`q`](Self::q).
    #[inline]
    pub fn qubit(&self, index: usize) -> Result<QubitHandle, BuilderError> {
        self.q(index)
    }

    /// Returns an iterator over all [`QubitHandle`]s in this register.
    pub fn iter(&self) -> impl Iterator<Item = QubitHandle> + '_ {
        self.handles.iter().copied()
    }

    /// Returns a slice of all [`QubitHandle`]s in this register.
    pub fn handles(&self) -> &[QubitHandle] {
        &self.handles
    }
}

impl Index<usize> for QuantumRegisterHandle {
    type Output = QubitHandle;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.handles[index]
    }
}

/// Handle to a declared classical register, providing safe indexing and bounds-checked access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassicalRegisterHandle {
    pub(crate) builder_id: u64,
    pub(crate) name: String,
    pub(crate) start_bit: usize,
    pub(crate) size: usize,
    pub(crate) handles: Vec<ClassicalBitHandle>,
}

impl ClassicalRegisterHandle {
    /// Creates a new `ClassicalRegisterHandle`.
    pub(crate) fn new(builder_id: u64, name: String, start_bit: usize, size: usize) -> Self {
        let mut handles = Vec::with_capacity(size);
        for i in 0..size {
            handles.push(ClassicalBitHandle::new(
                builder_id,
                ClassicalBitId(start_bit + i),
            ));
        }
        Self {
            builder_id,
            name,
            start_bit,
            size,
            handles,
        }
    }

    /// Returns the name of this classical register.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of classical bits in this register.
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns `true` if this register has size zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the base flat classical bit index in the circuit.
    #[inline]
    pub fn start_bit(&self) -> usize {
        self.start_bit
    }

    /// Returns the originating builder ID.
    #[inline]
    pub fn builder_id(&self) -> u64 {
        self.builder_id
    }

    /// Returns a validated [`ClassicalBitHandle`] for the bit at `index` in this register.
    ///
    /// # Errors
    /// Returns [`BuilderError::IndexOutOfBounds`] if `index >= self.size()`.
    pub fn c(&self, index: usize) -> Result<ClassicalBitHandle, BuilderError> {
        if index < self.size {
            Ok(self.handles[index])
        } else {
            Err(BuilderError::IndexOutOfBounds {
                register: self.name.clone(),
                index,
                size: self.size,
            })
        }
    }

    /// Convenient alias for [`c`](Self::c).
    #[inline]
    pub fn bit(&self, index: usize) -> Result<ClassicalBitHandle, BuilderError> {
        self.c(index)
    }

    /// Returns an iterator over all [`ClassicalBitHandle`]s in this register.
    pub fn iter(&self) -> impl Iterator<Item = ClassicalBitHandle> + '_ {
        self.handles.iter().copied()
    }

    /// Returns a slice of all [`ClassicalBitHandle`]s in this register.
    pub fn handles(&self) -> &[ClassicalBitHandle] {
        &self.handles
    }
}

impl Index<usize> for ClassicalRegisterHandle {
    type Output = ClassicalBitHandle;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.handles[index]
    }
}
