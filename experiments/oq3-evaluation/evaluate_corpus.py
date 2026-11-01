#!/usr/bin/env python3
"""OpenQASM 3 Parser Evaluation Script for QCC Phase 15.

Evaluates the pinned independent reference parser (`openqasm3[parser]==1.0.1`)
against the representative corpus of OpenQASM 3 programs.
"""

import glob
import json
import os
import sys

# Ensure pinned package path is included
sys.path.insert(0, "/tmp/py_packages")

try:
    import openqasm3
    import openqasm3.ast as ast
except ImportError as err:
    print(f"ERROR: Failed to import openqasm3: {err}")
    sys.exit(1)


def evaluate_file(filepath):
    filename = os.path.basename(filepath)
    with open(filepath, "r") as f:
        source = f.read()

    result = {
        "file": filename,
        "size_bytes": len(source),
        "lines": source.count("\n") + 1,
        "parse_success": False,
        "statement_count": 0,
        "statement_types": [],
        "spans_available": False,
        "error": None,
        "error_type": None,
        "error_line": None,
        "error_column": None,
    }

    try:
        parsed = openqasm3.parse(source)
        result["parse_success"] = True
        result["statement_count"] = len(parsed.statements)
        result["statement_types"] = [type(s).__name__ for s in parsed.statements]

        if parsed.statements:
            first_stmt = parsed.statements[0]
            if hasattr(first_stmt, "span") and first_stmt.span is not None:
                result["spans_available"] = True
                result["sample_span"] = {
                    "start_line": getattr(first_stmt.span, "start_line", None),
                    "start_column": getattr(first_stmt.span, "start_column", None),
                    "end_line": getattr(first_stmt.span, "end_line", None),
                    "end_column": getattr(first_stmt.span, "end_column", None),
                }

    except Exception as e:
        result["parse_success"] = False
        result["error"] = str(e)
        result["error_type"] = type(e).__name__
        if hasattr(e, "line"):
            result["error_line"] = e.line
        if hasattr(e, "column"):
            result["error_column"] = e.column

    return result


def main():
    corpus_dir = os.path.join(os.path.dirname(__file__), "corpus")
    qasm_files = sorted(glob.glob(os.path.join(corpus_dir, "*.qasm")))

    print("=" * 70)
    print(f"QCC OpenQASM 3 Evaluation: openqasm3 v{openqasm3.__version__}")
    print(f"Corpus directory: {corpus_dir}")
    print(f"Found {len(qasm_files)} test files")
    print("=" * 70)

    results = []
    for qasm_file in qasm_files:
        res = evaluate_file(qasm_file)
        results.append(res)
        status = "PASS" if res["parse_success"] else "FAIL/REJECT"
        print(f"[{status:11}] {res['file']:<34} | stmts: {res['statement_count']:<2} | err: {res['error_type']}")

    output_json = os.path.join(os.path.dirname(__file__), "py_parser_results.json")
    with open(output_json, "w") as f:
        json.dump(
            {
                "parser": "openqasm3[parser]",
                "version": openqasm3.__version__,
                "evaluation_date": "2026-11-01",
                "results": results,
            },
            f,
            indent=2,
        )

    print("-" * 70)
    print(f"Saved evaluation results to {output_json}")


if __name__ == "__main__":
    main()
