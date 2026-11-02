# OpenQASM 3 Frontend Fuzzing & Resource Bounds

This document specifies the fuzzing architecture, regression corpus management, and resource bounds enforced by the **QCC (Quantum Circuit Compiler)** OpenQASM 3 frontend, established in **Phase 27**.

---

## 1. Architectural Invariants for Malformed Inputs

To ensure stability, predictability, and safety against adversarial or corrupted inputs:
1. **Zero Panics**: The lexer, recovery parser, semantic analyzer, and circuit lowerer must **never panic** on any input.
2. **Bounded Recursion**: Recursive-descent Pratt expression parsing is strictly bounded to prevent thread stack exhaustion.
3. **Bounded Memory Allocations**: Register sizes, operand lists, parameter counts, and total token allocations are capped before memory is committed.
4. **Deterministic Failures**: Malformed inputs produce deterministic diagnostic codes with line and column information.

---

## 2. Configured Resource Limits

| Component | Limit Parameter | Default Value | Enforced In |
|---|---|:---:|---|
| Lexer | `max_source_bytes` | 16 MiB (16,777,216 B) | [`LexerLimits::max_source_bytes`](../src/frontend/lexer.rs) |
| Lexer | `max_tokens` | 500,000 tokens | [`LexerLimits::max_tokens`](../src/frontend/lexer.rs) |
| Lexer | `max_token_bytes` | 65,536 bytes | [`LexerLimits::max_token_bytes`](../src/frontend/lexer.rs) |
| Parser | `MAX_EXPRESSION_DEPTH` | 128 levels | [`ExpressionParser::depth`](../src/frontend/parser/expressions.rs) |
| Parser | `MAX_OPERANDS_PER_STATEMENT` | 10,000 operands | [`OperandParser::parse_qubit_operands_allow_unindexed`](../src/frontend/parser/operands.rs) |
| Parser | `MAX_PARAMETERS_PER_GATE` | 128 parameters | [`GateParser::parse_gate_statement`](../src/frontend/parser/gates.rs) |
| Parser | `MAX_REGISTER_SIZE` | 1,000,000 qubits/bits | [`DeclarationParser::parse_register_decl`](../src/frontend/parser/declarations.rs) |

---

## 3. Fuzz Targets

The fuzzing suite is maintained under [`fuzz/`](../fuzz/):

### 1. `fuzz_lexer` (`fuzz/fuzz_targets/fuzz_lexer.rs`)
Fuzzes the lexical scanner with arbitrary byte slices:
- Verifies UTF-8 and invalid byte sequence handling.
- Tests token bounds and oversized numeric literals.
- Asserts that token sequences always terminate with a valid EOF token.
- Asserts that every token span is strictly within source buffer bounds.

### 2. `fuzz_parser` (`fuzz/fuzz_targets/fuzz_parser.rs`)
Fuzzes the complete frontend pipeline (lexer $\to$ recovery parser $\to$ semantic analyzer $\to$ circuit lowerer):
- Tests deeply nested parenthesized expressions (`((((...))))`).
- Tests large operand lists (`barrier q[0], q[1], ...`).
- Tests division by zero, floating point overflows, and NaN propagation.
- Asserts that recovery AST statements maintain valid spans and that successful lowerings pass QccIR graph validation.

---

## 4. Running Fuzz Tests

### Running Standalone Smoke Runs (Stable Rust, No Extra Tools)
Both fuzz targets include built-in deterministic pseudo-random generators (`SplitMix64`) and synthetic edge-case runners that execute on standard stable Rust:

```bash
# Run lexer smoke fuzzing (1000 iterations + synthetic edge cases)
cargo run --bin fuzz_lexer -- --smoke

# Run parser smoke fuzzing (1000 iterations + synthetic edge cases)
cargo run --bin fuzz_parser -- --smoke

# Run against corpus files
cargo run --bin fuzz_lexer -- fuzz/corpus/lexer/
cargo run --bin fuzz_parser -- fuzz/corpus/parser/
```

### Running with `cargo-fuzz` / `libFuzzer` (Optional, Nightly)
For continuous fuzzing campaigns using LLVM's `libFuzzer`:

```bash
# Install cargo-fuzz (once)
cargo install cargo-fuzz

# Run continuous fuzzing
cargo +nightly fuzz run fuzz_lexer
cargo +nightly fuzz run fuzz_parser
```

---

## 5. Regression Corpus Management

Discovered crashes or interesting edge cases are stored permanently in [`fuzz/corpus/`](../fuzz/corpus/):
- `fuzz/corpus/lexer/`: Lexical edge cases (long literals, unclosed comments, huge whitespace).
- `fuzz/corpus/parser/`: Syntactic edge cases (deep nesting, division by zero, huge operands, malformed statements).

The entire regression corpus is executed automatically on every test run via [`tests/qasm3/fuzz_regression.rs`](../tests/qasm3/fuzz_regression.rs).
