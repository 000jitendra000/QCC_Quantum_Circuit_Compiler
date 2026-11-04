//! Physical connectivity, coupling graphs, and directionality for target operations.
//!
//! Enforces the rule that symmetric undirected couplings (such as $CZ$) and asymmetric
//! directed couplings (such as $CX(q_{\text{ctrl}}, q_{\text{targ}})$) are distinct and non-interchangeable.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::target::error::TargetError;
use crate::target::operation::PhysicalQubitId;

/// Directionality constraint of an operation coupling edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Directionality {
    /// Asymmetric directed coupling: $(u, v)$ does not imply $(v, u)$.
    Directed,
    /// Symmetric undirected coupling: $\{u, v\}$ permits both $(u, v)$ and $(v, u)$.
    Undirected,
}

impl fmt::Display for Directionality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directed => write!(f, "Directed"),
            Self::Undirected => write!(f, "Undirected"),
        }
    }
}

/// A coupling edge connecting two physical device qubits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CouplingEdge {
    /// First physical qubit operand (e.g. control for CX).
    pub source: PhysicalQubitId,
    /// Second physical qubit operand (e.g. target for CX).
    pub target: PhysicalQubitId,
}

impl CouplingEdge {
    /// Creates a new coupling edge.
    #[inline]
    pub const fn new(source: PhysicalQubitId, target: PhysicalQubitId) -> Self {
        Self { source, target }
    }

    /// Returns `true` if this edge connects a qubit to itself.
    #[inline]
    pub fn is_self_loop(&self) -> bool {
        self.source == self.target
    }

    /// Returns the reversed edge $(target, source)$.
    #[inline]
    pub fn reversed(&self) -> Self {
        Self {
            source: self.target,
            target: self.source,
        }
    }
}

impl fmt::Display for CouplingEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.source, self.target)
    }
}

impl From<(PhysicalQubitId, PhysicalQubitId)> for CouplingEdge {
    #[inline]
    fn from((src, dst): (PhysicalQubitId, PhysicalQubitId)) -> Self {
        Self::new(src, dst)
    }
}

impl From<(u32, u32)> for CouplingEdge {
    #[inline]
    fn from((src, dst): (u32, u32)) -> Self {
        Self::new(PhysicalQubitId::new(src), PhysicalQubitId::new(dst))
    }
}

/// Physical connectivity and allowed operand tuples for an operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationConnectivity {
    /// Single-qubit operation executable on designated physical qubits.
    SingleQubit {
        /// Permitted physical qubits.
        qubits: BTreeSet<PhysicalQubitId>,
    },
    /// Directed two-qubit operation executable only on designated ordered pairs $(u, v)$.
    DirectedTwoQubit {
        /// Permitted directed pairs (source, target).
        edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)>,
    },
    /// Undirected two-qubit operation executable symmetrically on $\{u, v\}$ in either order.
    UndirectedTwoQubit {
        /// Permitted undirected edges stored normalized as $(min(u, v), max(u, v))$.
        edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)>,
    },
    /// All-to-all connectivity: valid across all physical qubits (for 1Q) or all distinct pairs (for 2Q).
    AllToAll,
    /// General explicit multi-qubit operand tuples.
    General {
        /// Set of permitted ordered operand tuples.
        tuples: BTreeSet<Vec<PhysicalQubitId>>,
    },
}

impl OperationConnectivity {
    /// Creates a single-qubit connectivity descriptor for specific physical qubits.
    pub fn single_qubit(qubits: impl IntoIterator<Item = PhysicalQubitId>) -> Self {
        Self::SingleQubit {
            qubits: qubits.into_iter().collect(),
        }
    }

    /// Creates a single-qubit connectivity descriptor permitting all physical qubits up to `capacity`.
    pub fn all_single_qubits(capacity: usize) -> Self {
        let qubits = (0..capacity as u32).map(PhysicalQubitId::new).collect();
        Self::SingleQubit { qubits }
    }

    /// Creates a directed two-qubit connectivity descriptor from ordered pairs.
    pub fn directed_two_qubit(
        edges: impl IntoIterator<Item = (PhysicalQubitId, PhysicalQubitId)>,
    ) -> Self {
        Self::DirectedTwoQubit {
            edges: edges.into_iter().collect(),
        }
    }

    /// Creates an undirected two-qubit connectivity descriptor from pairs (normalized $u < v$).
    pub fn undirected_two_qubit(
        edges: impl IntoIterator<Item = (PhysicalQubitId, PhysicalQubitId)>,
    ) -> Self {
        let normalized = edges
            .into_iter()
            .map(|(u, v)| if u < v { (u, v) } else { (v, u) })
            .collect();
        Self::UndirectedTwoQubit { edges: normalized }
    }

    /// Creates an all-to-all connectivity descriptor.
    pub fn all_to_all() -> Self {
        Self::AllToAll
    }

    /// Creates a general connectivity descriptor from an arbitrary set of tuples.
    pub fn general(tuples: impl IntoIterator<Item = Vec<PhysicalQubitId>>) -> Self {
        Self::General {
            tuples: tuples.into_iter().collect(),
        }
    }

    /// Returns `true` if this connectivity enforces asymmetric operand directionality.
    #[inline]
    pub fn is_directed(&self) -> bool {
        matches!(self, Self::DirectedTwoQubit { .. })
    }

    /// Returns the directionality category.
    #[inline]
    pub fn directionality(&self) -> Directionality {
        if self.is_directed() {
            Directionality::Directed
        } else {
            Directionality::Undirected
        }
    }

    /// Checks if a physical operand tuple is valid and permitted under this connectivity.
    pub fn is_valid_operands(&self, operands: &[PhysicalQubitId]) -> bool {
        match self {
            Self::SingleQubit { qubits } => {
                if operands.len() != 1 {
                    return false;
                }
                qubits.contains(&operands[0])
            }
            Self::DirectedTwoQubit { edges } => {
                if operands.len() != 2 || operands[0] == operands[1] {
                    return false;
                }
                edges.contains(&(operands[0], operands[1]))
            }
            Self::UndirectedTwoQubit { edges } => {
                if operands.len() != 2 || operands[0] == operands[1] {
                    return false;
                }
                let norm = if operands[0] < operands[1] {
                    (operands[0], operands[1])
                } else {
                    (operands[1], operands[0])
                };
                edges.contains(&norm)
            }
            Self::AllToAll => {
                // All distinct qubits permitted
                for i in 0..operands.len() {
                    for j in (i + 1)..operands.len() {
                        if operands[i] == operands[j] {
                            return false;
                        }
                    }
                }
                true
            }
            Self::General { tuples } => tuples.contains(operands),
        }
    }

    /// Expands this connectivity into the complete set of permitted physical operand tuples.
    pub fn allowed_tuples(
        &self,
        target_capacity: usize,
        arity: usize,
    ) -> BTreeSet<Vec<PhysicalQubitId>> {
        let mut result = BTreeSet::new();
        match self {
            Self::SingleQubit { qubits } => {
                for &q in qubits {
                    if q.index() < target_capacity {
                        result.insert(vec![q]);
                    }
                }
            }
            Self::DirectedTwoQubit { edges } => {
                for &(u, v) in edges {
                    if u.index() < target_capacity && v.index() < target_capacity && u != v {
                        result.insert(vec![u, v]);
                    }
                }
            }
            Self::UndirectedTwoQubit { edges } => {
                for &(u, v) in edges {
                    if u.index() < target_capacity && v.index() < target_capacity && u != v {
                        result.insert(vec![u, v]);
                        result.insert(vec![v, u]);
                    }
                }
            }
            Self::AllToAll => {
                if arity == 1 {
                    for i in 0..target_capacity {
                        result.insert(vec![PhysicalQubitId::new(i as u32)]);
                    }
                } else if arity == 2 {
                    for i in 0..target_capacity {
                        for j in 0..target_capacity {
                            if i != j {
                                result.insert(vec![
                                    PhysicalQubitId::new(i as u32),
                                    PhysicalQubitId::new(j as u32),
                                ]);
                            }
                        }
                    }
                }
            }
            Self::General { tuples } => {
                for tup in tuples {
                    if tup.len() == arity && tup.iter().all(|q| q.index() < target_capacity) {
                        result.insert(tup.clone());
                    }
                }
            }
        }
        result
    }

    /// Validates that all physical qubit references and edges are within bounds and well-formed.
    pub fn validate(
        &self,
        op_name: &str,
        arity: usize,
        target_capacity: usize,
    ) -> Result<(), TargetError> {
        match self {
            Self::SingleQubit { qubits } => {
                if arity != 1 {
                    return Err(TargetError::InvalidArity {
                        name: op_name.to_string(),
                        target: "<descriptor>".to_string(),
                        arity,
                        capacity: target_capacity,
                    });
                }
                for &q in qubits {
                    if q.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: q,
                            target: op_name.to_string(),
                            capacity: target_capacity,
                        });
                    }
                }
            }
            Self::DirectedTwoQubit { edges } => {
                if arity != 2 {
                    return Err(TargetError::InvalidArity {
                        name: op_name.to_string(),
                        target: "<descriptor>".to_string(),
                        arity,
                        capacity: target_capacity,
                    });
                }
                for &(u, v) in edges {
                    if u.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: u,
                            target: op_name.to_string(),
                            capacity: target_capacity,
                        });
                    }
                    if v.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: v,
                            target: op_name.to_string(),
                            capacity: target_capacity,
                        });
                    }
                    if u == v {
                        return Err(TargetError::SelfLoopEdge {
                            operation: op_name.to_string(),
                            qubit: u,
                        });
                    }
                }
            }
            Self::UndirectedTwoQubit { edges } => {
                if arity != 2 {
                    return Err(TargetError::InvalidArity {
                        name: op_name.to_string(),
                        target: "<descriptor>".to_string(),
                        arity,
                        capacity: target_capacity,
                    });
                }
                for &(u, v) in edges {
                    if u.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: u,
                            target: op_name.to_string(),
                            capacity: target_capacity,
                        });
                    }
                    if v.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: v,
                            target: op_name.to_string(),
                            capacity: target_capacity,
                        });
                    }
                    if u == v {
                        return Err(TargetError::SelfLoopEdge {
                            operation: op_name.to_string(),
                            qubit: u,
                        });
                    }
                }
            }
            Self::AllToAll => {
                // Valid for any arity <= capacity
            }
            Self::General { tuples } => {
                for tup in tuples {
                    if tup.len() != arity {
                        return Err(TargetError::ArityMismatch {
                            operation: op_name.to_string(),
                            expected: arity,
                            actual: tup.len(),
                            operands: tup.clone(),
                        });
                    }
                    for &q in tup {
                        if q.index() >= target_capacity {
                            return Err(TargetError::QubitOutOfBounds {
                                qubit: q,
                                target: op_name.to_string(),
                                capacity: target_capacity,
                            });
                        }
                    }
                    for i in 0..tup.len() {
                        for j in (i + 1)..tup.len() {
                            if tup[i] == tup[j] {
                                return Err(TargetError::DuplicateOperandQubit {
                                    operation: op_name.to_string(),
                                    qubit: tup[i],
                                    operands: tup.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Physical coupling graph representation of a target device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CouplingGraph {
    /// Total number of physical qubits.
    pub num_qubits: usize,
    /// Edges between physical qubits.
    pub edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)>,
    /// Default directionality interpretation of the edges.
    pub directionality: Directionality,
}

impl CouplingGraph {
    /// Creates an empty coupling graph for the given physical qubit count.
    pub fn new(num_qubits: usize, directionality: Directionality) -> Self {
        Self {
            num_qubits,
            edges: BTreeSet::new(),
            directionality,
        }
    }

    /// Adds a coupling edge between two physical qubits.
    pub fn add_edge(
        &mut self,
        source: impl Into<PhysicalQubitId>,
        target: impl Into<PhysicalQubitId>,
    ) -> Result<(), TargetError> {
        let u = source.into();
        let v = target.into();

        if u.index() >= self.num_qubits {
            return Err(TargetError::QubitOutOfBounds {
                qubit: u,
                target: "<coupling_graph>".to_string(),
                capacity: self.num_qubits,
            });
        }
        if v.index() >= self.num_qubits {
            return Err(TargetError::QubitOutOfBounds {
                qubit: v,
                target: "<coupling_graph>".to_string(),
                capacity: self.num_qubits,
            });
        }
        if u == v {
            return Err(TargetError::SelfLoopEdge {
                operation: "<coupling_graph>".to_string(),
                qubit: u,
            });
        }

        self.edges.insert((u, v));
        if self.directionality == Directionality::Undirected {
            self.edges.insert((v, u));
        }
        Ok(())
    }

    /// Returns `true` if an edge exists from `source` to `target`.
    pub fn has_edge(&self, source: PhysicalQubitId, target: PhysicalQubitId) -> bool {
        self.edges.contains(&(source, target))
    }

    /// Returns the neighboring physical qubits accessible from `qubit`.
    pub fn neighbors(&self, qubit: PhysicalQubitId) -> Vec<PhysicalQubitId> {
        self.edges
            .iter()
            .filter(|(src, _)| *src == qubit)
            .map(|(_, dst)| *dst)
            .collect()
    }

    /// Returns the out-degree of `qubit`.
    pub fn degree(&self, qubit: PhysicalQubitId) -> usize {
        self.neighbors(qubit).len()
    }

    /// Returns all physical coupling edges.
    pub fn all_edges(&self) -> Vec<(PhysicalQubitId, PhysicalQubitId)> {
        self.edges.iter().copied().collect()
    }

    /// Checks if the coupling graph is fully connected (all physical qubits mutually reachable).
    pub fn is_connected(&self) -> bool {
        if self.num_qubits <= 1 {
            return true;
        }

        // Build undirected adjacency list for connectivity check
        let mut adj: BTreeMap<PhysicalQubitId, Vec<PhysicalQubitId>> = BTreeMap::new();
        for &(u, v) in &self.edges {
            adj.entry(u).or_default().push(v);
            adj.entry(v).or_default().push(u);
        }

        let start = PhysicalQubitId::new(0);
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();

        visited.insert(start);
        queue.push_back(start);

        while let Some(curr) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&curr) {
                for &next in neighbors {
                    if visited.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }

        visited.len() == self.num_qubits
    }

    /// Constructs a standard 1D linear chain topology: $0 - 1 - 2 - \dots - (n-1)$.
    pub fn linear_chain(num_qubits: usize, directionality: Directionality) -> Self {
        let mut graph = Self::new(num_qubits, directionality);
        for i in 0..(num_qubits.saturating_sub(1)) {
            let u = PhysicalQubitId::new(i as u32);
            let v = PhysicalQubitId::new((i + 1) as u32);
            let _ = graph.add_edge(u, v);
        }
        graph
    }

    /// Constructs a 2D grid topology with dimensions `rows` $\times$ `cols`.
    pub fn grid_2d(rows: usize, cols: usize, directionality: Directionality) -> Self {
        let num_qubits = rows * cols;
        let mut graph = Self::new(num_qubits, directionality);

        for r in 0..rows {
            for c in 0..cols {
                let u = PhysicalQubitId::new((r * cols + c) as u32);
                // Horizontal neighbor
                if c + 1 < cols {
                    let v = PhysicalQubitId::new((r * cols + (c + 1)) as u32);
                    let _ = graph.add_edge(u, v);
                }
                // Vertical neighbor
                if r + 1 < rows {
                    let v = PhysicalQubitId::new(((r + 1) * cols + c) as u32);
                    let _ = graph.add_edge(u, v);
                }
            }
        }

        graph
    }

    /// Constructs an all-to-all fully connected topology.
    pub fn all_to_all(num_qubits: usize, directionality: Directionality) -> Self {
        let mut graph = Self::new(num_qubits, directionality);
        for i in 0..num_qubits {
            for j in 0..num_qubits {
                if i != j {
                    let u = PhysicalQubitId::new(i as u32);
                    let v = PhysicalQubitId::new(j as u32);
                    let _ = graph.add_edge(u, v);
                }
            }
        }
        graph
    }

    /// Constructs a 1D ring (cycle) topology: $0 - 1 - 2 - \dots - (n-1) - 0$.
    pub fn ring(num_qubits: usize, directionality: Directionality) -> Self {
        let mut graph = Self::linear_chain(num_qubits, directionality);
        if num_qubits >= 3 {
            let u = PhysicalQubitId::new((num_qubits - 1) as u32);
            let v = PhysicalQubitId::new(0);
            let _ = graph.add_edge(u, v);
        }
        graph
    }

    /// Constructs a star topology with a central hub qubit connected to all other satellite qubits.
    pub fn star(num_qubits: usize, hub: usize, directionality: Directionality) -> Self {
        let mut graph = Self::new(num_qubits, directionality);
        if num_qubits > 1 && hub < num_qubits {
            let center = PhysicalQubitId::new(hub as u32);
            for i in 0..num_qubits {
                if i != hub {
                    let sat = PhysicalQubitId::new(i as u32);
                    let _ = graph.add_edge(center, sat);
                }
            }
        }
        graph
    }
}
