//! Example demonstrating OpenQASM 3 export from a QccIR Bell state circuit.
//!
//! Run with:
//! ```bash
//! cargo run --example qasm
//! ```

use std::fs;
use std::path::Path;

use qcc::backend::openqasm::emit_openqasm;
use qcc::builder::CircuitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== QCC OpenQASM 3 Emitter: Bell State Circuit Export ===\n");

    // 1. Build Bell state circuit using the public CircuitBuilder API
    let mut builder = CircuitBuilder::new("bell_state");
    let q = builder.add_quantum_register("q", 2)?;
    let c = builder.add_classical_register("c", 2)?;

    println!("Constructing Bell state preparation and measurement:");
    builder.h(q[0])?;
    println!("  h q[0]");
    builder.cx(q[0], q[1])?;
    println!("  cx q[0], q[1]");
    builder.measure(q[0], c[0])?;
    println!("  measure q[0] -> c[0]");
    builder.measure(q[1], c[1])?;
    println!("  measure q[1] -> c[1]");

    // 2. Finish and validate circuit
    let circuit = builder.finish()?;
    circuit.validate()?;
    println!("\nCircuit '{}' validated successfully.", circuit.name);

    // 3. Export to OpenQASM 3.0 source text
    let qasm_source = emit_openqasm(&circuit)?;

    println!("\nGenerated OpenQASM 3.0 Program:");
    println!("----------------------------------------");
    print!("{qasm_source}");
    println!("----------------------------------------");

    // 4. Save emitted artifact to examples/qasm/bell.qasm
    let output_path = Path::new("examples/qasm/bell.qasm");
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, &qasm_source)?;
    println!("\nOpenQASM 3 program saved to: {}", output_path.display());
    println!("=== OpenQASM 3 Export Complete ===");

    Ok(())
}
