//! Static quantum and classical register symbol table and operand resolver.
//!
//! Tracks declared registers in strict source declaration order, assigns stable typed IDs,
//! rejects invalid declarations (zero size, duplicate names, cross-type collisions), and
//! resolves indexed operands with bounds validation.

use std::collections::HashMap;
use std::fmt;

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::{AstBitRef, AstQubitRef, AstRegisterDecl, AstRegisterKind};

/// Strongly-typed identifier for a declared quantum register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuantumRegisterId(pub u32);

impl fmt::Display for QuantumRegisterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "qreg#{}", self.0)
    }
}

/// Strongly-typed identifier for a declared classical register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassicalRegisterId(pub u32);

impl fmt::Display for ClassicalRegisterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "creg#{}", self.0)
    }
}

/// Flat, 0-indexed logical qubit identifier assigned in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QubitId(pub u32);

impl fmt::Display for QubitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "q#{}", self.0)
    }
}

/// Flat, 0-indexed classical bit identifier assigned in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitId(pub u32);

impl fmt::Display for BitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c#{}", self.0)
    }
}

/// Classification of register domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegisterKind {
    /// Quantum qubit register (`qubit` or `qubit[n]`).
    Quantum,
    /// Classical bit register (`bit` or `bit[n]`).
    Classical,
}

impl fmt::Display for RegisterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quantum => write!(f, "quantum"),
            Self::Classical => write!(f, "classical"),
        }
    }
}

/// Entry for a declared quantum register.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantumRegisterEntry {
    /// Assigned strongly-typed quantum register identifier.
    pub id: QuantumRegisterId,
    /// Declared register name.
    pub name: String,
    /// Number of qubits in this register (1 for scalar declarations).
    pub size: usize,
    /// `true` if declared as scalar (`qubit q;`), `false` if declared as an array (`qubit[n] q;`).
    pub is_scalar: bool,
    /// Starting flat logical qubit index within the compilation unit.
    pub base_index: usize,
    /// Exact source span of the declaration statement.
    pub span: SourceSpan,
}

/// Entry for a declared classical register.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassicalRegisterEntry {
    /// Assigned strongly-typed classical register identifier.
    pub id: ClassicalRegisterId,
    /// Declared register name.
    pub name: String,
    /// Number of bits in this register (1 for scalar declarations).
    pub size: usize,
    /// `true` if declared as scalar (`bit c;`), `false` if declared as an array (`bit[n] c;`).
    pub is_scalar: bool,
    /// Starting flat classical bit index within the compilation unit.
    pub base_index: usize,
    /// Exact source span of the declaration statement.
    pub span: SourceSpan,
}

/// Polymorphic wrapper preserving source declaration order across quantum and classical registers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterEntry {
    /// Quantum register declaration.
    Quantum(QuantumRegisterEntry),
    /// Classical register declaration.
    Classical(ClassicalRegisterEntry),
}

impl RegisterEntry {
    /// Returns the declared name of this register.
    pub fn name(&self) -> &str {
        match self {
            Self::Quantum(r) => &r.name,
            Self::Classical(r) => &r.name,
        }
    }

    /// Returns the allocated size of this register.
    pub fn size(&self) -> usize {
        match self {
            Self::Quantum(r) => r.size,
            Self::Classical(r) => r.size,
        }
    }

    /// Returns `true` if declared as scalar.
    pub fn is_scalar(&self) -> bool {
        match self {
            Self::Quantum(r) => r.is_scalar,
            Self::Classical(r) => r.is_scalar,
        }
    }

    /// Returns the source span of the declaration.
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Quantum(r) => r.span,
            Self::Classical(r) => r.span,
        }
    }

    /// Returns the register kind.
    pub fn kind(&self) -> RegisterKind {
        match self {
            Self::Quantum(_) => RegisterKind::Quantum,
            Self::Classical(_) => RegisterKind::Classical,
        }
    }
}

/// Fully resolved qubit operand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedQubit {
    /// Identifier of the enclosing quantum register.
    pub register_id: QuantumRegisterId,
    /// Name of the enclosing quantum register.
    pub register_name: String,
    /// 0-indexed element within the register (0 for scalar registers).
    pub index_in_register: usize,
    /// Unique flat logical qubit index within the circuit.
    pub flat_qubit_id: QubitId,
    /// Source span of the operand reference.
    pub span: SourceSpan,
}

/// Fully resolved classical bit operand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBit {
    /// Identifier of the enclosing classical register.
    pub register_id: ClassicalRegisterId,
    /// Name of the enclosing classical register.
    pub register_name: String,
    /// 0-indexed element within the register (0 for scalar registers).
    pub index_in_register: usize,
    /// Unique flat classical bit index within the circuit.
    pub flat_bit_id: BitId,
    /// Source span of the operand reference.
    pub span: SourceSpan,
}

/// Errors produced during register declaration and operand resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// Register name is already declared in this scope (duplicate or collision).
    DuplicateName {
        /// Colliding identifier name.
        name: String,
        /// Source span of the duplicate declaration.
        span: SourceSpan,
        /// Source span of the earlier declaration if available.
        previous_span: Option<Box<SourceSpan>>,
    },
    /// Register declared with size 0.
    ZeroSize {
        /// Register name.
        name: String,
        /// Source span of the declaration.
        span: SourceSpan,
    },
    /// Reference to an undeclared register identifier.
    UndeclaredRegister {
        /// Unresolved name.
        name: String,
        /// Source span of the reference.
        span: SourceSpan,
    },
    /// Register referenced with the wrong type domain (e.g. classical bit used as qubit operand).
    TypeMismatch {
        /// Register name.
        name: String,
        /// Expected register type ("qubit" or "bit").
        expected: &'static str,
        /// Actual register type ("qubit" or "bit").
        actual: &'static str,
        /// Source span of the reference.
        span: SourceSpan,
    },
    /// Referenced element index is out of bounds for the register's declared size.
    IndexOutOfBounds {
        /// Register name.
        name: String,
        /// Requested element index.
        index: usize,
        /// Declared register size.
        size: usize,
        /// Source span of the operand reference.
        span: SourceSpan,
    },
    /// Attempted to index a scalar register declaration.
    IndexedScalar {
        /// Register name.
        name: String,
        /// Source span of the reference.
        span: SourceSpan,
    },
    /// Attempted to use an array register without an index.
    UnindexedArray {
        /// Register name.
        name: String,
        /// Declared size.
        size: usize,
        /// Source span of the reference.
        span: SourceSpan,
    },
}

impl RegisterError {
    /// Returns the primary source span associated with this error.
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::DuplicateName { span, .. } => *span,
            Self::ZeroSize { span, .. } => *span,
            Self::UndeclaredRegister { span, .. } => *span,
            Self::TypeMismatch { span, .. } => *span,
            Self::IndexOutOfBounds { span, .. } => *span,
            Self::IndexedScalar { span, .. } => *span,
            Self::UnindexedArray { span, .. } => *span,
        }
    }

    /// Converts this register error into a formatted compiler [`Diagnostic`].
    pub fn to_diagnostic(&self, file_path: &str) -> Diagnostic {
        match self {
            Self::DuplicateName {
                name,
                span,
                previous_span,
            } => {
                let mut diag = Diagnostic::error(format!(
                    "duplicate register declaration '{name}'; identifier was already declared"
                ))
                .with_code("E0010")
                .with_span(*span)
                .with_file_path(file_path);

                if let Some(prev) = previous_span {
                    diag = diag.with_note(format!("previously declared at line {}", prev.start_line()));
                }
                diag
            }
            Self::ZeroSize { name, span } => Diagnostic::error(format!(
                "register '{name}' has invalid size 0; register size must be positive"
            ))
            .with_code("E0011")
            .with_span(*span)
            .with_file_path(file_path),
            Self::UndeclaredRegister { name, span } => {
                Diagnostic::error(format!("use of undeclared register '{name}'"))
                    .with_code("E0012")
                    .with_span(*span)
                    .with_file_path(file_path)
            }
            Self::TypeMismatch {
                name,
                expected,
                actual,
                span,
            } => Diagnostic::error(format!(
                "register '{name}' is a {actual} register, but was used where a {expected} was expected"
            ))
            .with_code("E0013")
            .with_span(*span)
            .with_file_path(file_path),
            Self::IndexOutOfBounds {
                name,
                index,
                size,
                span,
            } => Diagnostic::error(format!(
                "index {index} out of bounds for register '{name}' of size {size} (valid index range is 0..{})",
                size.saturating_sub(1)
            ))
            .with_code("E0014")
            .with_span(*span)
            .with_file_path(file_path),
            Self::IndexedScalar { name, span } => Diagnostic::error(format!(
                "cannot index scalar register '{name}' with '[...]'"
            ))
            .with_code("E0015")
            .with_span(*span)
            .with_file_path(file_path),
            Self::UnindexedArray { name, size, span } => Diagnostic::error(format!(
                "array register '{name}' of size {size} must be indexed with '[index]'"
            ))
            .with_code("E0016")
            .with_span(*span)
            .with_file_path(file_path),
        }
    }
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateName { name, .. } => {
                write!(f, "duplicate register declaration '{name}'")
            }
            Self::ZeroSize { name, .. } => {
                write!(f, "register '{name}' has invalid size 0")
            }
            Self::UndeclaredRegister { name, .. } => {
                write!(f, "use of undeclared register '{name}'")
            }
            Self::TypeMismatch {
                name,
                expected,
                actual,
                ..
            } => {
                write!(
                    f,
                    "register '{name}' is a {actual} register, expected {expected}"
                )
            }
            Self::IndexOutOfBounds {
                name, index, size, ..
            } => {
                write!(
                    f,
                    "index {index} out of bounds for register '{name}' of size {size}"
                )
            }
            Self::IndexedScalar { name, .. } => {
                write!(f, "cannot index scalar register '{name}'")
            }
            Self::UnindexedArray { name, size, .. } => {
                write!(f, "array register '{name}' of size {size} must be indexed")
            }
        }
    }
}

impl std::error::Error for RegisterError {}

/// Symbol table tracking all declared quantum and classical registers in source order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterTable {
    quantum_registers: Vec<QuantumRegisterEntry>,
    classical_registers: Vec<ClassicalRegisterEntry>,
    declaration_order: Vec<RegisterEntry>,
    name_lookup: HashMap<String, (RegisterKind, usize)>,
    total_qubits: usize,
    total_bits: usize,
}

impl RegisterTable {
    /// Constructs a new, empty [`RegisterTable`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a new quantum register.
    ///
    /// # Errors
    /// Returns [`RegisterError::DuplicateName`] if `name` is already declared as a quantum or
    /// classical register.
    /// Returns [`RegisterError::ZeroSize`] if `size == Some(0)`.
    pub fn declare_quantum(
        &mut self,
        name: impl Into<String>,
        size: Option<usize>,
        span: SourceSpan,
    ) -> Result<QuantumRegisterId, RegisterError> {
        let name = name.into();

        if let Some((_, idx)) = self.name_lookup.get(&name) {
            let prev_span = self.declaration_order[*idx].span();
            return Err(RegisterError::DuplicateName {
                name,
                span,
                previous_span: Some(Box::new(prev_span)),
            });
        }

        if size == Some(0) {
            return Err(RegisterError::ZeroSize { name, span });
        }

        let is_scalar = size.is_none();
        let alloc_size = size.unwrap_or(1);
        let id = QuantumRegisterId(self.quantum_registers.len() as u32);
        let base_index = self.total_qubits;
        self.total_qubits += alloc_size;

        let entry = QuantumRegisterEntry {
            id,
            name: name.clone(),
            size: alloc_size,
            is_scalar,
            base_index,
            span,
        };

        let order_idx = self.declaration_order.len();
        self.declaration_order
            .push(RegisterEntry::Quantum(entry.clone()));
        self.quantum_registers.push(entry);
        self.name_lookup
            .insert(name, (RegisterKind::Quantum, order_idx));

        Ok(id)
    }

    /// Declares a new classical register.
    ///
    /// # Errors
    /// Returns [`RegisterError::DuplicateName`] if `name` is already declared as a quantum or
    /// classical register.
    /// Returns [`RegisterError::ZeroSize`] if `size == Some(0)`.
    pub fn declare_classical(
        &mut self,
        name: impl Into<String>,
        size: Option<usize>,
        span: SourceSpan,
    ) -> Result<ClassicalRegisterId, RegisterError> {
        let name = name.into();

        if let Some((_, idx)) = self.name_lookup.get(&name) {
            let prev_span = self.declaration_order[*idx].span();
            return Err(RegisterError::DuplicateName {
                name,
                span,
                previous_span: Some(Box::new(prev_span)),
            });
        }

        if size == Some(0) {
            return Err(RegisterError::ZeroSize { name, span });
        }

        let is_scalar = size.is_none();
        let alloc_size = size.unwrap_or(1);
        let id = ClassicalRegisterId(self.classical_registers.len() as u32);
        let base_index = self.total_bits;
        self.total_bits += alloc_size;

        let entry = ClassicalRegisterEntry {
            id,
            name: name.clone(),
            size: alloc_size,
            is_scalar,
            base_index,
            span,
        };

        let order_idx = self.declaration_order.len();
        self.declaration_order
            .push(RegisterEntry::Classical(entry.clone()));
        self.classical_registers.push(entry);
        self.name_lookup
            .insert(name, (RegisterKind::Classical, order_idx));

        Ok(id)
    }

    /// Declares a register from an AST declaration node.
    pub fn declare_ast(&mut self, decl: &AstRegisterDecl) -> Result<(), RegisterError> {
        match decl.kind {
            AstRegisterKind::Qubit => {
                self.declare_quantum(&decl.name, decl.size, decl.span)?;
            }
            AstRegisterKind::Bit => {
                self.declare_classical(&decl.name, decl.size, decl.span)?;
            }
        }
        Ok(())
    }

    /// Resolves an AST qubit operand reference against declared quantum registers.
    ///
    /// # Errors
    /// Returns an error if the register is undeclared, is a classical bit register,
    /// is scalar but indexed, is an array but unindexed, or has an out-of-bounds index.
    pub fn resolve_qubit(&self, qubit_ref: &AstQubitRef) -> Result<ResolvedQubit, RegisterError> {
        let (kind, order_idx) = self.name_lookup.get(&qubit_ref.name).ok_or_else(|| {
            RegisterError::UndeclaredRegister {
                name: qubit_ref.name.clone(),
                span: qubit_ref.span,
            }
        })?;

        if *kind != RegisterKind::Quantum {
            return Err(RegisterError::TypeMismatch {
                name: qubit_ref.name.clone(),
                expected: "qubit",
                actual: "bit",
                span: qubit_ref.span,
            });
        }

        let entry = match &self.declaration_order[*order_idx] {
            RegisterEntry::Quantum(e) => e,
            _ => unreachable!(),
        };

        let index_in_reg = if entry.is_scalar {
            if qubit_ref.index.is_some() {
                return Err(RegisterError::IndexedScalar {
                    name: qubit_ref.name.clone(),
                    span: qubit_ref.span,
                });
            }
            0
        } else {
            match qubit_ref.index {
                None => {
                    return Err(RegisterError::UnindexedArray {
                        name: qubit_ref.name.clone(),
                        size: entry.size,
                        span: qubit_ref.span,
                    })
                }
                Some(idx) => {
                    if idx >= entry.size {
                        return Err(RegisterError::IndexOutOfBounds {
                            name: qubit_ref.name.clone(),
                            index: idx,
                            size: entry.size,
                            span: qubit_ref.span,
                        });
                    }
                    idx
                }
            }
        };

        let flat_qubit_id = QubitId((entry.base_index + index_in_reg) as u32);
        Ok(ResolvedQubit {
            register_id: entry.id,
            register_name: entry.name.clone(),
            index_in_register: index_in_reg,
            flat_qubit_id,
            span: qubit_ref.span,
        })
    }

    /// Resolves an AST classical bit operand reference against declared classical registers.
    ///
    /// # Errors
    /// Returns an error if the register is undeclared, is a quantum qubit register,
    /// is scalar but indexed, is an array but unindexed, or has an out-of-bounds index.
    pub fn resolve_bit(&self, bit_ref: &AstBitRef) -> Result<ResolvedBit, RegisterError> {
        let (kind, order_idx) = self.name_lookup.get(&bit_ref.name).ok_or_else(|| {
            RegisterError::UndeclaredRegister {
                name: bit_ref.name.clone(),
                span: bit_ref.span,
            }
        })?;

        if *kind != RegisterKind::Classical {
            return Err(RegisterError::TypeMismatch {
                name: bit_ref.name.clone(),
                expected: "bit",
                actual: "qubit",
                span: bit_ref.span,
            });
        }

        let entry = match &self.declaration_order[*order_idx] {
            RegisterEntry::Classical(e) => e,
            _ => unreachable!(),
        };

        let index_in_reg = if entry.is_scalar {
            if bit_ref.index.is_some() {
                return Err(RegisterError::IndexedScalar {
                    name: bit_ref.name.clone(),
                    span: bit_ref.span,
                });
            }
            0
        } else {
            match bit_ref.index {
                None => {
                    return Err(RegisterError::UnindexedArray {
                        name: bit_ref.name.clone(),
                        size: entry.size,
                        span: bit_ref.span,
                    })
                }
                Some(idx) => {
                    if idx >= entry.size {
                        return Err(RegisterError::IndexOutOfBounds {
                            name: bit_ref.name.clone(),
                            index: idx,
                            size: entry.size,
                            span: bit_ref.span,
                        });
                    }
                    idx
                }
            }
        };

        let flat_bit_id = BitId((entry.base_index + index_in_reg) as u32);
        Ok(ResolvedBit {
            register_id: entry.id,
            register_name: entry.name.clone(),
            index_in_register: index_in_reg,
            flat_bit_id,
            span: bit_ref.span,
        })
    }

    /// Returns a slice of all declared registers in exact source declaration order.
    pub fn declaration_order(&self) -> &[RegisterEntry] {
        &self.declaration_order
    }

    /// Returns a slice of all declared quantum registers in declaration order.
    pub fn quantum_registers(&self) -> &[QuantumRegisterEntry] {
        &self.quantum_registers
    }

    /// Returns a slice of all declared classical registers in declaration order.
    pub fn classical_registers(&self) -> &[ClassicalRegisterEntry] {
        &self.classical_registers
    }

    /// Looks up a quantum register by identifier name.
    pub fn get_quantum(&self, name: &str) -> Option<&QuantumRegisterEntry> {
        let (kind, order_idx) = self.name_lookup.get(name)?;
        if *kind == RegisterKind::Quantum {
            match &self.declaration_order[*order_idx] {
                RegisterEntry::Quantum(e) => Some(e),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Looks up a classical register by identifier name.
    pub fn get_classical(&self, name: &str) -> Option<&ClassicalRegisterEntry> {
        let (kind, order_idx) = self.name_lookup.get(name)?;
        if *kind == RegisterKind::Classical {
            match &self.declaration_order[*order_idx] {
                RegisterEntry::Classical(e) => Some(e),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Looks up a quantum register by typed ID.
    pub fn get_quantum_by_id(&self, id: QuantumRegisterId) -> Option<&QuantumRegisterEntry> {
        self.quantum_registers.get(id.0 as usize)
    }

    /// Looks up a classical register by typed ID.
    pub fn get_classical_by_id(&self, id: ClassicalRegisterId) -> Option<&ClassicalRegisterEntry> {
        self.classical_registers.get(id.0 as usize)
    }

    /// Returns the total number of individual qubits allocated across all quantum registers.
    pub fn total_qubits(&self) -> usize {
        self.total_qubits
    }

    /// Returns the total number of individual bits allocated across all classical registers.
    pub fn total_bits(&self) -> usize {
        self.total_bits
    }

    /// Returns `true` if no registers have been declared.
    pub fn is_empty(&self) -> bool {
        self.declaration_order.is_empty()
    }

    /// Returns the total count of declared registers.
    pub fn len(&self) -> usize {
        self.declaration_order.len()
    }
}
