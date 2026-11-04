//! Reproducible baseline routing benchmark corpus (Phase 49).
//!
//! Provides a standardized corpus of representative quantum circuits and target hardware
//! topologies to evaluate router performance and establish baseline quality metrics
//! for future routing algorithms (such as SABRE).
//!
//! Each benchmark fixture uses:
//! - Deterministic circuit construction.
//! - Known target topology and coupling constraints.
//! - Explicit placement strategy and explicit seeds.
//! - Comprehensive metric recording (SWAP count, DAG depth, 2Q depth, gate overhead, cost).

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::routing::greedy::{BaselineRouter, BaselineRouterConfig};
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::metrics::RoutingMetrics;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::swap_insertion::PathSwapStrategy;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};

/// Definition of a benchmark fixture in the reproducible baseline routing corpus.
pub struct BenchmarkFixture {
    /// Unique identifier for the benchmark.
    pub name: &'static str,
    /// Detailed description of the benchmark circuit and purpose.
    pub description: &'static str,
    /// Logical quantum circuit to be routed.
    pub circuit: Circuit,
    /// Physical hardware target device.
    pub target: Target,
    /// Router configuration specifying placement and SWAP strategies.
    pub config: BaselineRouterConfig,
    /// Optional pre-determined initial layout.
    pub initial_layout: Option<LayoutMap>,
}

/// Helper building a target device with single-qubit gates, native `cx`, and optional native `swap`.
fn make_target_topology(
    name: &str,
    num_qubits: usize,
    edges: impl IntoIterator<Item = (u32, u32)>,
    native_swap: bool,
) -> Target {
    let mut target = Target::new(name, num_qubits);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(h_desc);

    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(x_desc);

    let z_desc = TargetOperationDescriptor::builder("z", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(10.0)
        .build();
    target.add_operation(z_desc);

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

/// Constructs the canonical baseline benchmark corpus.
pub fn build_baseline_corpus() -> Vec<BenchmarkFixture> {
    let mut corpus = Vec::new();

    // 1. Line 5-Qubit Distance-4 Interaction
    {
        let mut b = CircuitBuilder::new("line_5q_dist4");
        let q = b.add_quantum_register("q", 5).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[4]).unwrap();
        let circuit = b.finish().unwrap();

        let edges = (0..4).map(|i| (i, i + 1));
        let target = make_target_topology("line_5", 5, edges, true);

        corpus.push(BenchmarkFixture {
            name: "line_5q_dist4",
            description: "5-qubit linear chain with distance-4 interaction between q[0] and q[4]",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Identity,
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: Some(LayoutMap::identity(5, 5).unwrap()),
        });
    }

    // 2. Line 10-Qubit Nested Long-Range Interactions
    {
        let mut b = CircuitBuilder::new("line_10q_nested");
        let q = b.add_quantum_register("q", 10).unwrap();
        b.h(q[0]).unwrap();
        b.h(q[1]).unwrap();
        b.h(q[2]).unwrap();
        b.cx(q[0], q[9]).unwrap();
        b.cx(q[1], q[8]).unwrap();
        b.cx(q[2], q[7]).unwrap();
        let circuit = b.finish().unwrap();

        let edges = (0..9).map(|i| (i, i + 1));
        let target = make_target_topology("line_10", 10, edges, true);

        corpus.push(BenchmarkFixture {
            name: "line_10q_nested",
            description: "10-qubit linear chain with nested concentric long-range CX gates",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Weighted,
                swap_strategy: PathSwapStrategy::Bidirectional,
                ..BaselineRouterConfig::default()
            },
            initial_layout: None,
        });
    }

    // 3. Ring 6-Qubit Wraparound Interaction
    {
        let mut b = CircuitBuilder::new("ring_6q_wraparound");
        let q = b.add_quantum_register("q", 6).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[5]).unwrap();
        let circuit = b.finish().unwrap();

        let mut edges: Vec<(u32, u32)> = (0..5).map(|i| (i, i + 1)).collect();
        edges.push((5, 0)); // Ring wraparound coupling
        let target = make_target_topology("ring_6", 6, edges, true);

        corpus.push(BenchmarkFixture {
            name: "ring_6q_wraparound",
            description: "6-qubit ring coupling where shortest path exploits the wraparound edge",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Identity,
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: Some(LayoutMap::identity(6, 6).unwrap()),
        });
    }

    // 4. Grid 2x3 Cross-Corner Interaction
    {
        let mut b = CircuitBuilder::new("grid_2x3_diagonal");
        let q = b.add_quantum_register("q", 6).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[5]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        let circuit = b.finish().unwrap();

        // 2x3 grid edges:
        // 0 - 1 - 2
        // |   |   |
        // 3 - 4 - 5
        let edges = vec![(0, 1), (1, 2), (3, 4), (4, 5), (0, 3), (1, 4), (2, 5)];
        let target = make_target_topology("grid_2x3", 6, edges, true);

        corpus.push(BenchmarkFixture {
            name: "grid_2x3_diagonal",
            description: "6-qubit 2x3 planar grid with cross-corner interactions",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Weighted,
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: None,
        });
    }

    // 5. Star 5-Qubit Hub-and-Spoke Leaf-to-Leaf Interaction
    {
        let mut b = CircuitBuilder::new("star_5q_leaves");
        let q = b.add_quantum_register("q", 5).unwrap();
        b.cx(q[1], q[3]).unwrap();
        b.cx(q[2], q[4]).unwrap();
        let circuit = b.finish().unwrap();

        // Hub = 0, Leaves = 1, 2, 3, 4
        let edges = vec![(0, 1), (0, 2), (0, 3), (0, 4)];
        let target = make_target_topology("star_5", 5, edges, true);

        corpus.push(BenchmarkFixture {
            name: "star_5q_leaves",
            description: "5-qubit star topology requiring leaf-to-leaf routing through central hub",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Degree,
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: None,
        });
    }

    // 6. Bell State with ADR-0001 Idle Ancillae
    {
        let mut b = CircuitBuilder::new("bell_adr0001_ancillae");
        let q = b.add_quantum_register("q", 2).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[1]).unwrap();
        let circuit = b.finish().unwrap();

        // 4-qubit line with 2 logical qubits and 2 idle physical ancillae
        let edges = vec![(0, 1), (1, 2), (2, 3)];
        let target = make_target_topology("line_4_ancillae", 4, edges, true);

        corpus.push(BenchmarkFixture {
            name: "bell_adr0001_ancillae",
            description: "2-qubit Bell circuit on 4-qubit hardware verifying ADR-0001 zero-state ancilla tracking",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Weighted,
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: None,
        });
    }

    // 7. Parallel CX Layers (Testing DAG Depth and Concurrency)
    {
        let mut b = CircuitBuilder::new("parallel_cx_layers");
        let q = b.add_quantum_register("q", 6).unwrap();
        // Layer 1: disjoint parallel CXs (0,1), (2,3), (4,5)
        b.cx(q[0], q[1]).unwrap();
        b.cx(q[2], q[3]).unwrap();
        b.cx(q[4], q[5]).unwrap();
        // Layer 2: interleaved parallel CXs (1,2), (3,4)
        b.cx(q[1], q[2]).unwrap();
        b.cx(q[3], q[4]).unwrap();
        let circuit = b.finish().unwrap();

        let edges = (0..5).map(|i| (i, i + 1));
        let target = make_target_topology("line_6", 6, edges, true);

        corpus.push(BenchmarkFixture {
            name: "parallel_cx_layers",
            description: "6-qubit line with two alternating parallel entangling layers",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Identity,
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: Some(LayoutMap::identity(6, 6).unwrap()),
        });
    }

    // 8. Seeded Randomized Placement Reproducibility
    {
        let mut b = CircuitBuilder::new("seeded_random_placement");
        let q = b.add_quantum_register("q", 6).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[2]).unwrap();
        b.cx(q[1], q[4]).unwrap();
        b.cx(q[3], q[5]).unwrap();
        let circuit = b.finish().unwrap();

        let edges = (0..5).map(|i| (i, i + 1));
        let target = make_target_topology("line_6_seed", 6, edges, true);

        corpus.push(BenchmarkFixture {
            name: "seeded_random_placement",
            description:
                "6-qubit random placement with explicit seed 424242 ensuring reproducible layout",
            circuit,
            target,
            config: BaselineRouterConfig {
                placement_strategy: PlacementStrategy::Random { seed: 424_242 },
                swap_strategy: PathSwapStrategy::SourceToTarget,
                ..BaselineRouterConfig::default()
            },
            initial_layout: None,
        });
    }

    corpus
}

/// Executes all benchmarks in the baseline corpus, displaying structured results.
pub fn run_baseline_benchmarks() -> Vec<(&'static str, RoutingMetrics)> {
    let corpus = build_baseline_corpus();
    let mut results = Vec::new();

    println!("\n=========================================================================================");
    println!("               QCC Phase 49: Baseline Router Benchmark Corpus Results");
    println!(
        "========================================================================================="
    );
    println!(
        "{:<25} | {:>4} | {:>4} | {:>5} | {:>5} | {:>5} | {:>8} | {:>8}",
        "Benchmark Name", "LogQ", "Phys", "SWAPs", "Depth", "2QDep", "Cost", "Time (µs)"
    );
    println!(
        "-----------------------------------------------------------------------------------------"
    );

    for fixture in corpus {
        let router = BaselineRouter::with_config(fixture.config.clone());
        let (routed, report) = router
            .route_circuit(&fixture.circuit, &fixture.target, fixture.initial_layout)
            .unwrap_or_else(|err| {
                panic!(
                    "benchmark fixture '{}' failed routing: {err:?}",
                    fixture.name
                )
            });

        // Verify routed circuit is non-empty and valid
        assert!(routed.num_operations() >= fixture.circuit.num_operations());
        assert_eq!(report.swaps_inserted, report.metrics.routing_swaps_count);

        let micros = report
            .metrics
            .run_duration
            .map(|d| d.as_micros())
            .unwrap_or(0);

        println!(
            "{:<25} | {:>4} | {:>4} | {:>5} | {:>5} | {:>5} | {:>8.1} | {:>8}",
            fixture.name,
            fixture.circuit.num_qubits(),
            fixture.target.num_qubits(),
            report.metrics.routing_swaps_count,
            report.metrics.circuit_depth,
            report.metrics.two_qubit_depth,
            report.metrics.weighted_cost,
            micros
        );

        results.push((fixture.name, report.metrics));
    }

    println!("=========================================================================================\n");
    results
}

fn main() {
    let results = run_baseline_benchmarks();
    let results2 = run_baseline_benchmarks();

    assert_eq!(results.len(), results2.len());
    for ((name1, m1), (name2, m2)) in results.iter().zip(results2.iter()) {
        assert_eq!(name1, name2);
        assert!(
            m1.is_deterministic_match(m2),
            "fixture '{name1}' was not deterministic across repeated runs",
        );
    }

    println!(
        "Successfully evaluated and verified {} baseline benchmark fixtures (reproducibility verified).",
        results.len()
    );
}
