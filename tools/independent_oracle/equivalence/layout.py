"""Little-endian layout validation, physical embedding, and ADR-0001 ancilla checks.

Conventions (QCC Invariants & ADR-0001 / ADR-0003):
- Qubit indexing is strictly little-endian (`q[0]` is LSB, bit index 0).
- Manifest layouts map logical qubit index `j in 0..num_logical-1` ->
  physical qubit index `p in 0..num_physical-1` (`mapping_direction = "logical_to_physical"`).
- Initial embedding maps `|k>_logical` (2^n) into `|pi_init(k)>_physical` (2^m),
  strictly setting all physical qubits outside `im(L_init)` to |0>.
- Final embedding maps `|k'>_logical` (2^n) into `|pi_final(k')>_physical` (2^m),
  strictly setting all physical qubits in `unused_physical_qubits_final` to |0>.
- Unused physical qubits are NEVER traced out.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping

import numpy as np


@dataclass(frozen=True)
class ValidatedLayout:
    """Validated initial and final injective logical-to-physical mappings."""

    num_logical: int
    num_physical: int
    initial_logical_to_physical: dict[int, int]
    final_logical_to_physical: dict[int, int]
    unused_physical_initial: tuple[int, ...]
    unused_physical_final: tuple[int, ...]
    ancilla_mask_final: int


def parse_and_validate_single_layout(
    mapping: Mapping[str | int, int],
    *,
    num_logical: int,
    num_physical: int,
    expected_unused: list[int] | tuple[int, ...] | None = None,
    label: str = "layout",
) -> tuple[dict[int, int], tuple[int, ...]]:
    """Validate an injective, complete logical-to-physical map."""
    if num_logical < 0 or num_physical < 0:
        raise ValueError(f"{label}: qubit counts must be non-negative")
    if num_logical > num_physical:
        raise ValueError(
            f"{label}: logical_qubits ({num_logical}) exceeds physical_qubits ({num_physical})"
        )

    parsed: dict[int, int] = {}
    for raw_k, raw_v in mapping.items():
        if isinstance(raw_k, bool) or isinstance(raw_v, bool):
            raise ValueError(f"{label}: boolean index is invalid")
        try:
            k = int(raw_k)
            v = int(raw_v)
        except (TypeError, ValueError) as exc:
            raise ValueError(f"{label}: non-integer layout entry {raw_k!r}: {raw_v!r}") from exc
        if k in parsed:
            raise ValueError(f"{label}: duplicate logical qubit {k}")
        if not (0 <= k < num_logical):
            raise ValueError(
                f"{label}: logical qubit {k} out of range 0..{num_logical - 1}"
            )
        if not (0 <= v < num_physical):
            raise ValueError(
                f"{label}: physical qubit {v} out of range 0..{num_physical - 1}"
            )
        parsed[k] = v

    missing_logical = [j for j in range(num_logical) if j not in parsed]
    if missing_logical:
        raise ValueError(f"{label}: missing logical qubits {missing_logical}")

    phys_targets = list(parsed.values())
    if len(set(phys_targets)) != len(phys_targets):
        raise ValueError(f"{label}: mapping is not injective ({parsed})")

    unused = tuple(p for p in range(num_physical) if p not in set(phys_targets))
    if expected_unused is not None:
        if sorted(expected_unused) != list(unused):
            raise ValueError(
                f"{label}: unused physical qubits mismatch: manifest={list(expected_unused)}, "
                f"computed={list(unused)}"
            )
    return dict(sorted(parsed.items())), unused


def validate_manifest_layout(manifest: dict) -> ValidatedLayout:
    """Validate and extract both initial and final layouts from a QCC manifest."""
    layout_obj = manifest.get("layout")
    if not isinstance(layout_obj, dict):
        raise ValueError("Manifest contains no target layout (frontend_only or null)")

    if layout_obj.get("mapping_direction") != "logical_to_physical":
        raise ValueError(
            f"Unsupported mapping_direction: {layout_obj.get('mapping_direction')!r}"
        )
    if layout_obj.get("qubit_indexing_convention") != "little_endian_q0_lsb":
        raise ValueError(
            f"Unsupported qubit_indexing_convention: {layout_obj.get('qubit_indexing_convention')!r}"
        )

    num_logical = int(manifest["logical_qubits"])
    num_physical = int(manifest["physical_qubits"])
    if int(layout_obj["num_logical"]) != num_logical:
        raise ValueError(
            f"Layout num_logical ({layout_obj['num_logical']}) != manifest logical_qubits ({num_logical})"
        )
    if int(layout_obj["num_physical"]) != num_physical:
        raise ValueError(
            f"Layout num_physical ({layout_obj['num_physical']}) != manifest physical_qubits ({num_physical})"
        )

    init_map, unused_init = parse_and_validate_single_layout(
        layout_obj["initial_logical_to_physical"],
        num_logical=num_logical,
        num_physical=num_physical,
        expected_unused=layout_obj.get("unused_physical_qubits_initial"),
        label="initial_logical_to_physical",
    )
    final_map, unused_final = parse_and_validate_single_layout(
        layout_obj["final_logical_to_physical"],
        num_logical=num_logical,
        num_physical=num_physical,
        expected_unused=layout_obj.get("unused_physical_qubits_final"),
        label="final_logical_to_physical",
    )

    ancilla_mask = 0
    for p in unused_final:
        ancilla_mask |= 1 << p

    return ValidatedLayout(
        num_logical=num_logical,
        num_physical=num_physical,
        initial_logical_to_physical=init_map,
        final_logical_to_physical=final_map,
        unused_physical_initial=unused_init,
        unused_physical_final=unused_final,
        ancilla_mask_final=ancilla_mask,
    )


def map_logical_basis_index(
    logical_idx: int, layout_map: Mapping[int, int], num_logical: int
) -> int:
    """Map little-endian logical basis index -> little-endian physical basis index.

    Logical qubit `j` (bit `j` of `logical_idx`) moves to physical bit `layout_map[j]`.
    All unmapped physical qubits remain bit 0 (|0>).
    """
    phys_idx = 0
    for logical_qubit in range(num_logical):
        if (logical_idx >> logical_qubit) & 1:
            phys_idx |= 1 << layout_map[logical_qubit]
    return phys_idx


def build_basis_embedding_indices(
    layout_map: Mapping[int, int], num_logical: int
) -> np.ndarray:
    """Return length-2^n array `perm` where `perm[k] = pi(k)`."""
    dim_log = 1 << num_logical
    indices = np.arange(dim_log, dtype=np.int64)
    phys = np.zeros(dim_log, dtype=np.int64)
    for logical_qubit in range(num_logical):
        phys |= ((indices >> logical_qubit) & 1) << layout_map[logical_qubit]
    return phys


def embed_logical_statevector(
    logical_state: np.ndarray,
    layout_map: Mapping[int, int],
    *,
    num_logical: int,
    num_physical: int,
) -> np.ndarray:
    """Embed a 2^n logical statevector into 2^m physical space with |0> ancillas."""
    dim_log = 1 << num_logical
    dim_phys = 1 << num_physical
    vec = np.asarray(logical_state, dtype=np.complex128)
    if vec.shape != (dim_log,):
        raise ValueError(
            f"Expected logical statevector shape ({dim_log},), got {vec.shape}"
        )
    perm = build_basis_embedding_indices(layout_map, num_logical)
    phys_vec = np.zeros(dim_phys, dtype=np.complex128)
    phys_vec[perm] = vec
    return phys_vec


def embed_logical_columns(
    logical_matrix: np.ndarray,
    layout_map: Mapping[int, int],
    *,
    num_logical: int,
    num_physical: int,
) -> np.ndarray:
    """Embed a 2^n x K matrix of logical state columns into 2^m x K physical space."""
    dim_log = 1 << num_logical
    dim_phys = 1 << num_physical
    mat = np.asarray(logical_matrix, dtype=np.complex128)
    if mat.ndim != 2 or mat.shape[0] != dim_log:
        raise ValueError(
            f"Expected logical matrix with {dim_log} rows, got shape {mat.shape}"
        )
    perm = build_basis_embedding_indices(layout_map, num_logical)
    out = np.zeros((dim_phys, mat.shape[1]), dtype=np.complex128)
    out[perm, :] = mat
    return out


def compute_residual_ancilla_population(
    physical_state: np.ndarray, unused_physical_final: tuple[int, ...]
) -> float:
    """Compute total probability weight where any unused physical qubit is in |1>.

    Detects both residual ancilla |1> excitation and ancilla-logical entanglement
    without tracing out the spare subsystem.
    """
    if not unused_physical_final:
        return 0.0
    vec = np.asarray(physical_state, dtype=np.complex128)
    dim_phys = vec.shape[0]
    mask = 0
    for p in unused_physical_final:
        mask |= 1 << p
    indices = np.arange(dim_phys, dtype=np.int64)
    dirty_rows = (indices & mask) != 0
    return float(np.sum(np.abs(vec[dirty_rows]) ** 2))


def compute_max_residual_ancilla_population_matrix(
    physical_columns: np.ndarray, unused_physical_final: tuple[int, ...]
) -> float:
    """Return maximum residual ancilla population across all columns of a 2^m x K matrix."""
    if not unused_physical_final:
        return 0.0
    mat = np.asarray(physical_columns, dtype=np.complex128)
    dim_phys = mat.shape[0]
    mask = 0
    for p in unused_physical_final:
        mask |= 1 << p
    indices = np.arange(dim_phys, dtype=np.int64)
    dirty_rows = (indices & mask) != 0
    col_pops = np.sum(np.abs(mat[dirty_rows, :]) ** 2, axis=0)
    return float(np.max(col_pops)) if col_pops.size > 0 else 0.0
