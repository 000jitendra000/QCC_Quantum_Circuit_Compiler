//! Demonstrates creating and validating a Bell state circuit using the public CircuitBuilder API.
//!
//! Run with: `cargo run --example bell`

use std::error::Error;

use qcc::builder::CircuitView;
use qcc::CircuitBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== QCC Circuit Builder: Bell State Preparation ===\n");

    // 1. Initialize CircuitBuilder
    let mut builder = CircuitBuilder::new("bell_state");

    // 2. Declare quantum and classical registers
    let q = builder.add_quantum_register("q", 2)?;
    let c = builder.add_classical_register("c", 2)?;

    println!("Declared registers:");
    println!("  Quantum register:   '{}' (size {})", q.name(), q.size());
    println!("  Classical register: '{}' (size {})\n", c.name(), c.size());

    // 3. Apply gates: H on q[0], CX on (control: q[0], target: q[1])
    let h_op = builder.h(q[0])?;
    println!("Applied H gate to {} (OperationId: {})", q[0], h_op);

    let cx_op = builder.cx(q[0], q[1])?;
    println!(
        "Applied CX gate from control {} to target {} (OperationId: {})",
        q[0], q[1], cx_op
    );

    // 4. Measure qubits into classical bits
    let m0 = builder.measure(q[0], c[0])?;
    let m1 = builder.measure(q[1], c[1])?;
    println!("Measured {} -> {} (OperationId: {})", q[0], c[0], m0);
    println!("Measured {} -> {} (OperationId: {})\n", q[1], c[1], m1);

    // 5. Inspect circuit before finishing using the immutable view
    println!("In-progress builder view summary:");
    print!("{}", builder.view());

    // 6. Finish and validate the circuit
    let circuit = builder.finish()?;
    println!(
        "\nSuccessfully finished and validated circuit '{}'!",
        circuit.name
    );

    // 7. Demonstrate the finalized circuit using CircuitView
    let view = CircuitView::new(&circuit);
    println!("\nFinalized circuit view:");
    println!("  Total qubits:         {}", view.num_qubits());
    println!("  Total classical bits: {}", view.num_classical_bits());
    println!("  Total operations:     {}", view.num_operations());
    println!("  Total dependencies:   {}", view.num_dependencies());

    println!("\nOperations in topological execution order:");
    for (i, node) in view.topological_operations()?.iter().enumerate() {
        println!("  Step {}: [{}] {}", i + 1, node.id, node.operation);
    }

    println!("\n=== Bell State Circuit Complete ===");
    Ok(())
}
