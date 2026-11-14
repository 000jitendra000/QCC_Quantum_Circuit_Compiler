# Phase Completion Report Template

> Use this template to document the verified completion of a roadmap milestone or phase before updating `docs/project-state/progress.md`. Milestone reports are consolidated under `docs/project-state/reports/milestone-mXX.md`.

---

# Phase Completion Report: Phase [NUM] — [TITLE]

## 1. Summary
- **Phase Number**: Phase [NUM]
- **Exact Phase Title**: [TITLE]
- **Milestone**: Milestone [ID]: [NAME]
- **Completion Date**: [YYYY-MM-DD]
- **Engineer / Agent Session**: [Session Identifier]

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `file1.rs` - [Brief description]
  - `tests/file1_test.rs` - [Brief description]
- **Modified Files**:
  - `file2.rs` - [Brief description of changes]

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**: [Exact text from roadmap]
- **Verification Command Executed**:
  ```bash
  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  ```
- **Execution Log / Output**:
  ```text
  [Paste relevant passing test summary]
  ```
- **Exit Gate Pass Confirmation**: [Confirmed / Verified / Pass]

## 4. Test Suite Summary
- **Unit Tests Added**: [Count and brief description]
- **Integration Tests Added**: [Count and brief description]
- **Property / Invariant Tests**: [Count and description]
- **Regression Check**: [All previous tests pass with zero failures]

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (no nightly).
- [x] Correct qubit endianness ($q_0$ is LSB).
- [x] Correct gate operand ordering (e.g. $CX(q_c, q_t)$).
- [x] Explicit global phase preserved.
- [x] Wire dependencies and graph invariants maintained.
- [x] Zero extraneous feature development (scope adhered strictly to phase boundary).

## 6. Known Limitations & Technical Debt
- [List any bounded limitations or areas deferred to future phases]
- [Ensure no unapproved architecture drifts exist]

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.

