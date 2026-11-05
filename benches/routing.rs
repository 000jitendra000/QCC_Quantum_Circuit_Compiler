//! Reproducible router comparative benchmark corpus (Phase 53).
//!
//! Evaluates and compares `BaselineRouter` and `SabreBidirectionalRouter` on an identical
//! standardized corpus of representative quantum circuits and hardware topologies.
//!
//! Metrics evaluated:
//! - Inserted routing SWAPs
//! - Circuit DAG depth
//! - Two-qubit gate depth
//! - Composite weighted cost
//! - Hardware execution duration (when calibrated)
//! - Wall-clock execution runtime (µs)
//!
//! Every benchmark execution validates QccIR structure, target legality, and layout-aware
//! semantic equivalence under ADR-0001 clean-ancilla invariants.

use std::time::Instant;

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::circuit::Circuit;
use qcc::quantum::angle::Angle;
use qcc::routing::greedy::{BaselineRouter, BaselineRouterConfig};
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::metrics::RoutingMetrics;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::sabre::{SabreBidirectionalRouter, SabreRouterConfig};
use qcc::routing::swap_insertion::PathSwapStrategy;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::legality::TargetLegalityValidator;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;

/// Single benchmark fixture definition in the comparative routing corpus.
pub struct ComparativeFixture {
    pub name: &'static str,
    pub description: &'static str,
    pub circuit: Circuit,
    pub target: Target,
    pub initial_layout: Option<LayoutMap>,
    pub baseline_config: BaselineRouterConfig,
    pub sabre_config: SabreRouterConfig,
}

/// Helper constructing a hardware target device.
fn make_target_topology(
    name: &str,
    num_qubits: usize,
    edges: impl IntoIterator<Item = (u32, u32)>,
    native_swap: bool,
) -> Target {
    let mut target = Target::new(name, num_qubits);

    for op_name in &["h", "x", "y", "z", "s", "sdg", "t", "tdg"] {
        let desc = TargetOperationDescriptor::builder(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(20.0)
            .build();
        target.add_operation(desc);
    }

    for op_name in &["rx", "ry", "rz"] {
        let desc = TargetOperationDescriptor::builder(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .duration(25.0)
            .build();
        target.add_operation(desc);
    }

    let edge_tuples: Vec<(PhysicalQubitId, PhysicalQubitId)> = edges
        .into_iter()
        .map(|(u, v)| (PhysicalQubitId::new(u), PhysicalQubitId::new(v)))
        .collect();

    if !edge_tuples.is_empty() {
        let cx_desc = TargetOperationDescriptor::builder("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(
                edge_tuples.clone(),
            ))
            .duration(150.0)
            .build();
        target.add_operation(cx_desc);

        if native_swap {
            let swap_desc = TargetOperationDescriptor::builder("swap", 2)
                .connectivity(OperationConnectivity::undirected_two_qubit(edge_tuples))
                .duration(300.0)
                .build();
            target.add_operation(swap_desc);
        }
    }

    target.validate().unwrap();
    target
}

/// Helper constructing a linear coupling target.
fn make_line_target(name: &str, num_qubits: usize) -> Target {
    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((i as u32, (i + 1) as u32));
    }
    make_target_topology(name, num_qubits, edges, false)
}

/// Helper constructing a ring coupling target.
fn make_ring_target(name: &str, num_qubits: usize) -> Target {
    let mut edges = Vec::new();
    for i in 0..num_qubits {
        let next = (i + 1) % num_qubits;
        edges.push((i as u32, next as u32));
    }
    make_target_topology(name, num_qubits, edges, false)
}

/// Helper constructing a 2D grid target.
fn make_grid_target(name: &str, rows: usize, cols: usize) -> Target {
    let num_qubits = rows * cols;
    let mut edges = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let u = (r * cols + c) as u32;
            if c + 1 < cols {
                edges.push((u, (r * cols + (c + 1)) as u32));
            }
            if r + 1 < rows {
                edges.push((u, ((r + 1) * cols + c) as u32));
            }
        }
    }
    make_target_topology(name, num_qubits, edges, false)
}

/// Helper constructing a star target (center 0, leaves 1..n-1).
fn make_star_target(name: &str, num_qubits: usize) -> Target {
    let mut edges = Vec::new();
    for i in 1..num_qubits {
        edges.push((0u32, i as u32));
    }
    make_target_topology(name, num_qubits, edges, false)
}

/// Constructs the standardized comparative routing corpus (10 fixtures).
pub fn build_comparative_corpus() -> Vec<ComparativeFixture> {
    let mut corpus = Vec::new();

    let default_base_cfg = BaselineRouterConfig {
        placement_strategy: PlacementStrategy::Weighted,
        swap_strategy: PathSwapStrategy::SourceToTarget,
        max_swaps: 1_000,
        max_iterations: 10_000,
        verify_equivalence: true,
        validate_target_legality: true,
        ..Default::default()
    };

    let default_sabre_cfg = SabreRouterConfig {
        refinement_rounds: 3,
        max_iterations: 1_000,
        random_seed: Some(42),
        heuristic_config: qcc::routing::sabre::SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    // 1. Line 5-Qubit Distance-4 Interaction
    {
        let mut b = CircuitBuilder::new("line_5q_dist4");
        let q = b.add_quantum_register("q", 5).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[4]).unwrap();
        b.cx(q[1], q[3]).unwrap();
        b.cx(q[0], q[4]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "line_5q_dist4",
            description: "5-qubit line with repeated distance-4 interactions",
            circuit,
            target: make_line_target("line_5q", 5),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 2. Line 10-Qubit Nested Interactions
    {
        let mut b = CircuitBuilder::new("line_10q_nested");
        let q = b.add_quantum_register("q", 10).unwrap();
        for i in 0..5 {
            b.cx(q[i], q[9 - i]).unwrap();
        }
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "line_10q_nested",
            description: "10-qubit line with concentric nested CX pairs",
            circuit,
            target: make_line_target("line_10q", 10),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 3. Ring 6-Qubit Wraparound Interaction
    {
        let mut b = CircuitBuilder::new("ring_6q_wraparound");
        let q = b.add_quantum_register("q", 6).unwrap();
        b.cx(q[0], q[5]).unwrap();
        b.cx(q[1], q[4]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "ring_6q_wraparound",
            description: "6-qubit ring with wraparound shortest-path interactions",
            circuit,
            target: make_ring_target("ring_6q", 6),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 4. Grid 2x3 Diagonal Interactions
    {
        let mut b = CircuitBuilder::new("grid_2x3_diagonal");
        let q = b.add_quantum_register("q", 6).unwrap();
        b.cx(q[0], q[1]).unwrap();
        b.cx(q[1], q[2]).unwrap();
        b.cx(q[0], q[2]).unwrap(); // triangle clique
        b.cx(q[0], q[5]).unwrap(); // opposite corners
        b.cx(q[2], q[3]).unwrap(); // opposite corners
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "grid_2x3_diagonal",
            description: "2x3 grid with diagonal corner and triangle interactions",
            circuit,
            target: make_grid_target("grid_2x3", 2, 3),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 5. Star 5-Qubit Hub Contention
    {
        let mut b = CircuitBuilder::new("star_5q_leaves");
        let q = b.add_quantum_register("q", 5).unwrap();
        b.cx(q[1], q[2]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        b.cx(q[3], q[4]).unwrap();
        b.cx(q[4], q[1]).unwrap();
        b.cx(q[0], q[1]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "star_5q_leaves",
            description: "5-qubit star topology with inter-leaf contention across hub",
            circuit,
            target: make_star_target("star_5q", 5),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 6. Parallel CX Layers
    {
        let mut b = CircuitBuilder::new("parallel_cx_layers");
        let q = b.add_quantum_register("q", 6).unwrap();
        // Layer 1
        b.cx(q[0], q[1]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        b.cx(q[4], q[5]).unwrap();
        // Layer 2
        b.cx(q[1], q[2]).unwrap();
        b.cx(q[3], q[4]).unwrap();
        // Layer 3
        b.cx(q[0], q[5]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "parallel_cx_layers",
            description: "6-qubit alternating parallel CX layers",
            circuit,
            target: make_line_target("line_6q", 6),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 7. Bell Circuit with ADR-0001 Clean Ancillae
    {
        let mut b = CircuitBuilder::new("bell_adr0001_ancillae");
        let q = b.add_quantum_register("q", 2).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[1]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "bell_adr0001_ancillae",
            description: "2 logical qubits routed onto 6-qubit line with 4 clean ancillae",
            circuit,
            target: make_line_target("line_6q", 6),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 8. 4-Qubit Quantum Fourier Transform Interaction Pattern
    {
        let mut b = CircuitBuilder::new("qft_4q_line");
        let q = b.add_quantum_register("q", 4).unwrap();
        // QFT pattern: H on q[i] followed by controlled rotations to q[j] (j > i)
        for i in 0..4 {
            b.h(q[i]).unwrap();
            for j in (i + 1)..4 {
                b.cx(q[i], q[j]).unwrap();
                b.rz(q[j], Angle::from_radians(0.392)).unwrap();
                b.cx(q[i], q[j]).unwrap();
            }
        }
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "qft_4q_line",
            description: "4-qubit QFT interaction pattern on 4-qubit line",
            circuit,
            target: make_line_target("line_4q", 4),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 9. 5-Qubit GHZ State on Star Target
    {
        let mut b = CircuitBuilder::new("ghz_5q_star");
        let q = b.add_quantum_register("q", 5).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[1]).unwrap();
        b.cx(q[1], q[2]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        b.cx(q[3], q[4]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "ghz_5q_star",
            description: "5-qubit cascaded GHZ state preparation on star target",
            circuit,
            target: make_star_target("star_5q", 5),
            initial_layout: None,
            baseline_config: default_base_cfg.clone(),
            sabre_config: default_sabre_cfg.clone(),
        });
    }

    // 10. Seeded Dense Random Circuit
    {
        let mut b = CircuitBuilder::new("dense_random_6q");
        let q = b.add_quantum_register("q", 6).unwrap();
        // Alternating multi-wire CX gates
        b.cx(q[0], q[3]).unwrap();
        b.cx(q[1], q[4]).unwrap();
        b.cx(q[2], q[5]).unwrap();
        b.cx(q[0], q[4]).unwrap();
        b.cx(q[1], q[5]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        b.cx(q[3], q[5]).unwrap();
        b.cx(q[0], q[2]).unwrap();
        let circuit = b.finish().unwrap();

        corpus.push(ComparativeFixture {
            name: "dense_random_6q",
            description: "6-qubit dense interaction pattern on 6-qubit ring",
            circuit,
            target: make_ring_target("ring_6q", 6),
            initial_layout: None,
            baseline_config: default_base_cfg,
            sabre_config: default_sabre_cfg,
        });
    }

    corpus
}

/// Execution record containing comparative routing metrics.
pub struct ComparisonRecord {
    pub name: &'static str,
    pub num_logical: usize,
    pub num_physical: usize,
    pub base_swaps: usize,
    pub sabre_swaps: usize,
    pub base_depth: usize,
    pub sabre_depth: usize,
    pub base_2q_depth: usize,
    pub sabre_2q_depth: usize,
    pub base_cost: f64,
    pub sabre_cost: f64,
    pub base_micros: u128,
    pub sabre_micros: u128,
    pub base_metrics: RoutingMetrics,
    pub sabre_metrics: RoutingMetrics,
}

/// Runs the comparative benchmark corpus and prints formatted results.
pub fn run_comparative_benchmarks() -> Vec<ComparisonRecord> {
    let corpus = build_comparative_corpus();
    let mut records = Vec::new();

    println!("\n=====================================================================================================================");
    println!(
        "                           QCC Phase 53: Comparative Router Benchmark Corpus Results"
    );
    println!("                                   (Baseline Shortest-Path vs SABRE Bidirectional)");
    println!("=====================================================================================================================");
    println!(
        "{:<22} | {:>4} | {:>4} | {:>9} | {:>10} | {:>8} | {:>9} | {:>9} | {:>10} | {:>8} | {:>9}",
        "Benchmark Name",
        "LogQ",
        "Phys",
        "Base SWAP",
        "SABRE SWAP",
        "Base Dep",
        "SABRE Dep",
        "Base Cost",
        "SABRE Cost",
        "Base µs",
        "SABRE µs"
    );
    println!("---------------------------------------------------------------------------------------------------------------------");

    for fixture in corpus {
        let validator = TargetLegalityValidator::new(fixture.target.clone());
        let equiv_checker = LayoutEquivalenceChecker::default();

        // 1. Run Baseline Router
        let base_start = Instant::now();
        let base_router = BaselineRouter::with_config(fixture.baseline_config.clone());
        let (base_routed, base_report) = base_router
            .route_circuit(
                &fixture.circuit,
                &fixture.target,
                fixture.initial_layout.clone(),
            )
            .unwrap_or_else(|e| panic!("Baseline failed on '{}': {e:?}", fixture.name));
        let base_elapsed = base_start.elapsed().as_micros();

        // Validate Baseline
        assert!(validator.validate_circuit(&base_routed).is_ok());
        assert!(
            equiv_checker
                .check_layout_equivalence(&fixture.circuit, &base_routed, &base_report.layout)
                .unwrap()
                .is_equivalent
        );

        // 2. Run SABRE Bidirectional Router
        let sabre_start = Instant::now();
        let sabre_router = SabreBidirectionalRouter::with_config(fixture.sabre_config.clone());
        let (sabre_routed, sabre_report) = sabre_router
            .route_circuit(
                &fixture.circuit,
                &fixture.target,
                fixture.initial_layout.clone(),
            )
            .unwrap_or_else(|e| panic!("SABRE failed on '{}': {e:?}", fixture.name));
        let sabre_elapsed = sabre_start.elapsed().as_micros();

        // Validate SABRE
        assert!(validator.validate_circuit(&sabre_routed).is_ok());
        assert!(
            equiv_checker
                .check_layout_equivalence(&fixture.circuit, &sabre_routed, &sabre_report.layout)
                .unwrap()
                .is_equivalent
        );

        let record = ComparisonRecord {
            name: fixture.name,
            num_logical: fixture.circuit.num_qubits(),
            num_physical: fixture.target.num_qubits(),
            base_swaps: base_report.swaps_inserted,
            sabre_swaps: sabre_report.swaps_inserted,
            base_depth: base_report.metrics.circuit_depth,
            sabre_depth: sabre_report.metrics.circuit_depth,
            base_2q_depth: base_report.metrics.two_qubit_depth,
            sabre_2q_depth: sabre_report.metrics.two_qubit_depth,
            base_cost: base_report.metrics.weighted_cost,
            sabre_cost: sabre_report.metrics.weighted_cost,
            base_micros: base_elapsed,
            sabre_micros: sabre_elapsed,
            base_metrics: base_report.metrics,
            sabre_metrics: sabre_report.metrics,
        };

        println!(
            "{:<22} | {:>4} | {:>4} | {:>9} | {:>10} | {:>8} | {:>9} | {:>9.1} | {:>10.1} | {:>8} | {:>9}",
            record.name,
            record.num_logical,
            record.num_physical,
            record.base_swaps,
            record.sabre_swaps,
            record.base_depth,
            record.sabre_depth,
            record.base_cost,
            record.sabre_cost,
            record.base_micros,
            record.sabre_micros,
        );

        records.push(record);
    }

    println!("=====================================================================================================================\n");
    records
}

fn main() {
    let run1 = run_comparative_benchmarks();
    let run2 = run_comparative_benchmarks();

    assert_eq!(run1.len(), run2.len());
    for (r1, r2) in run1.iter().zip(run2.iter()) {
        assert_eq!(r1.name, r2.name);
        assert!(
            r1.base_metrics.is_deterministic_match(&r2.base_metrics),
            "Baseline fixture '{}' was not deterministic",
            r1.name
        );
        assert!(
            r1.sabre_metrics.is_deterministic_match(&r2.sabre_metrics),
            "SABRE fixture '{}' was not deterministic",
            r1.name
        );
    }

    println!(
        "Successfully evaluated and verified {} comparative benchmark fixtures (100% deterministic reproducibility).",
        run1.len()
    );
}
