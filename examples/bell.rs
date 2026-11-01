//! Bell State Vertical Slice Demonstration (Milestone M1 Acceptance)
//!
//! Demonstrates the complete end-to-end QCC compilation, simulation, sampling,
//! and OpenQASM 3 export pipeline:
//!
//! CircuitBuilder -> QccIR -> Validation -> StateVector Simulation -> Seeded Sampling -> OpenQASM 3 Emitter
//!
//! Run with:
//! ```bash
//! cargo run --example bell
//! ```

use std::error::Error;

use qcc::backend::openqasm::emit_openqasm;
use qcc::builder::CircuitView;
use qcc::quantum::semantics::{format_basis_state, ClassicalBitOrder};
use qcc::simulator::executor::{execute_circuit, CircuitExecutor};
use qcc::simulator::sampling::SplitMix64;
use qcc::CircuitBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    println!("============================================================");
    println!("     QCC Milestone M1: Bell State Vertical Slice            ");
    println!("============================================================\n");

    // ------------------------------------------------------------------------
    // Step 1: Circuit Construction via Public CircuitBuilder API
    // ------------------------------------------------------------------------
    println!("[Step 1] Constructing Bell state circuit via CircuitBuilder API...");
    let mut builder = CircuitBuilder::new("bell_state");

    let q = builder.add_quantum_register("q", 2)?;
    let c = builder.add_classical_register("c", 2)?;

    println!(
        "  Declared quantum register:   '{}' (size {})",
        q.name(),
        q.size()
    );
    println!(
        "  Declared classical register: '{}' (size {})",
        c.name(),
        c.size()
    );

    let h_op = builder.h(q[0])?;
    println!("  Applied H gate on {} ({})", q[0], h_op);

    let cx_op = builder.cx(q[0], q[1])?;
    println!(
        "  Applied CX gate (ctrl: {}, targ: {}) ({})",
        q[0], q[1], cx_op
    );

    let m0 = builder.measure(q[0], c[0])?;
    let m1 = builder.measure(q[1], c[1])?;
    println!(
        "  Applied terminal measurement {} -> {} ({})",
        q[0], c[0], m0
    );
    println!(
        "  Applied terminal measurement {} -> {} ({})",
        q[1], c[1], m1
    );

    // ------------------------------------------------------------------------
    // Step 2: Circuit Finalization & QccIR Validation
    // ------------------------------------------------------------------------
    println!("\n[Step 2] Validating QccIR dependency DAG...");
    let circuit = builder.finish()?;
    circuit.validate()?;

    let view = CircuitView::new(&circuit);
    println!("  Circuit '{}' validated successfully:", circuit.name);
    println!("    Total qubits:         {}", view.num_qubits());
    println!("    Total classical bits: {}", view.num_classical_bits());
    println!("    Total operations:     {}", view.num_operations());
    println!("    Total dependencies:   {}", view.num_dependencies());

    println!("\n  Topological execution order:");
    for (i, node) in view.topological_operations()?.iter().enumerate() {
        println!("    Step {}: [{}] {}", i + 1, node.id, node.operation);
    }

    // ------------------------------------------------------------------------
    // Step 3: Statevector Simulation
    // ------------------------------------------------------------------------
    println!("\n[Step 3] Simulating circuit in statevector engine...");
    let (state, trace) = execute_circuit(&circuit)?;

    println!("  Evolved StateVector (dimension = {}):", state.len());
    println!("  (Little-endian convention: q[0] is LSB, basis is |q1 q0>)");
    for idx in 0..state.len() {
        let amp = state.get(idx).unwrap();
        let basis_label = format_basis_state(idx, 2);
        let prob = amp.norm_sqr();
        println!(
            "    Index {idx} ({basis_label}): amplitude = {:+.6} {:+.6}i, prob = {:.4}",
            amp.re, amp.im, prob
        );
    }
    println!("  Statevector $\\ell_2$ norm = {:.10}", state.norm());
    println!("  Executed operation trace steps = {}", trace.steps.len());

    // ------------------------------------------------------------------------
    // Step 4: Seeded Terminal Measurement Sampling
    // ------------------------------------------------------------------------
    let num_shots = 2_000;
    println!("\n[Step 4] Drawing {num_shots} terminal measurement samples (SplitMix64 seed 42)...");
    let executor = CircuitExecutor::new();
    let (result, _) = executor.sample(&circuit, num_shots, SplitMix64::new(42))?;

    let counts = result.counts(ClassicalBitOrder::MsbFirst);
    println!("  Measurement Histogram (MSB-first bit order: c[1]c[0]):");
    for (bitstring, count) in &counts {
        let freq = *count as f64 / num_shots as f64;
        let pct = freq * 100.0;
        let bar_len = (freq * 40.0).round() as usize;
        let bar: String = "█".repeat(bar_len);
        println!("    |{bitstring}> : {count:4} shots ({pct:5.1}%) |{bar}");
    }

    // ------------------------------------------------------------------------
    // Step 5: Deterministic OpenQASM 3.0 Export
    // ------------------------------------------------------------------------
    println!("\n[Step 5] Emitting OpenQASM 3.0 representation...");
    let qasm_source = emit_openqasm(&circuit)?;
    println!("------------------------------------------------------------");
    print!("{qasm_source}");
    println!("------------------------------------------------------------");

    println!("\n=== Milestone M1 Bell Vertical Slice Complete ===");
    Ok(())
}
