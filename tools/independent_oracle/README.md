# QCC Independent Verification Oracle Tooling (`tools/independent_oracle/`)

This directory houses the optional Python/Qiskit verification oracle and its versioned interface contracts defined by **Roadmap v3.1 (Milestone M7, Phases 60–70)**.

> [!NOTE]
> **Status — M7 Session 1 Complete (Contract & Manifest Schema)**:
> - [`schema/compile_manifest.schema.json`](./schema/compile_manifest.schema.json) defines `qcc.compile_manifest` (`1.0.0`), the machine-readable interface produced by `qcc compile --emit-manifest <FILE>`.
> - See [`docs/decisions/ADR-0003-independent-oracle-and-manifest-contract.md`](../../docs/decisions/ADR-0003-independent-oracle-and-manifest-contract.md) for the oracle boundary, supported unitary domain, layout direction (`logical_to_physical`), little-endian convention (`q[0]` LSB), global-phase rules, and clean-ancilla policy (ADR-0001).
> - Pinned Python/Qiskit runners, compatibility probes, comparators, corpus generators, and CI workflows are scoped for **M7 Sessions 2–5**.
> - Core `cargo build`, `cargo test`, and `qcc` runtime execution remain strictly independent of Python and Qiskit.

