"""Session 5 CI integration and final M7 acceptance gate tests."""

from __future__ import annotations

import pathlib
import sys
import unittest

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from ci_check import validate_all_schemas_and_committed_artifacts  # noqa: E402


class TestCiIntegrationAndSchemas(unittest.TestCase):
    """Verify CI workflow isolation, triggers, artifact preservation, and schemas."""

    def test_all_seven_schemas_and_committed_artifacts_validate(self) -> None:
        counts = validate_all_schemas_and_committed_artifacts()
        self.assertEqual(counts["schemas"], 7)
        self.assertEqual(counts["session2_manifests"], 5)
        self.assertEqual(counts["canonical_cases"], 10)
        self.assertGreaterEqual(counts["corpus_cases"], 1000)
        self.assertEqual(counts["benchmark_trials"], 180)

    def test_rust_ci_workflow_preserved_and_free_of_qiskit(self) -> None:
        ci_text = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        self.assertNotIn("qiskit", ci_text.lower())
        self.assertIn("cargo fmt --check", ci_text)
        self.assertIn("cargo clippy --all-targets -- -D warnings", ci_text)
        self.assertIn("cargo test", ci_text)

    def test_oracle_ci_workflow_is_isolated_pinned_and_uploads_artifacts(self) -> None:
        wf_path = REPO_ROOT / ".github" / "workflows" / "independent-oracle.yml"
        self.assertTrue(wf_path.exists())
        wf_text = wf_path.read_text(encoding="utf-8")

        for required in (
            "pull_request:",
            "push:",
            "schedule:",
            "workflow_dispatch:",
            "oracle-core-gate:",
            "oracle-campaign-and-baseline:",
            "needs: oracle-core-gate",
            "python-version: '3.12'",
            "tools/independent_oracle/requirements.txt",
            "ci_check.py",
            "--stage core",
            "--stage campaign",
            "actions/upload-artifact@v4",
            "if: failure()",
        ):
            self.assertIn(required, wf_text, msg=f"Missing {required!r} in independent-oracle.yml")

        self.assertNotIn("continue-on-error: true", wf_text)

    def test_cargo_toml_has_zero_python_or_qiskit_dependencies(self) -> None:
        cargo = (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8").lower()
        for forbidden in ("pyo3", "python", "qiskit", "openqasm3"):
            self.assertNotIn(forbidden, cargo)


if __name__ == "__main__":
    unittest.main(verbosity=2)

