"""Compact selected-V1 edge-case native caller oracle."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zlib

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "conformance/oblique_saved_caller_native.py"
FIXTURE = ROOT / "conformance/oblique-v1-edge-native.json"
CASES = (
    "edge_linear_duplicates_sub5",
    "edge_curve_duplicates_sub5",
    "edge_both_axis_gate",
    "edge_curve_exact5",
    "edge_alternating_short_long",
    "edge_tool1_same_mesh",
    "edge_tool3_same_mesh",
)

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group(required=True)
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
parser.add_argument("--case", choices=CASES, help="write or check one edge case")
args = parser.parse_args()


def compact_case(case_id: str) -> dict:
    env = os.environ.copy()
    pythonpath = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = str(ROOT / "scratch/apk-analysis-runtime/python") + (os.pathsep + pythonpath if pythonpath else "")
    command = [
        sys.executable,
        str(RUNNER),
        "--print-capture",
        "--capture-transitions",
        "--case",
        case_id,
        "--redraw-version",
        "selected",
    ]
    completed = subprocess.run(command, cwd=ROOT, env=env, check=True, capture_output=True, text=True)
    native = json.loads(completed.stdout)
    native_calls = native["native_calls"]
    if native_calls["route"]["drawing_selected_drawable_version"] != "1":
        raise RuntimeError(f"{case_id}: expected selected Oblique V1")
    if native["input"]["advanced_setting"] is not None:
        raise RuntimeError(f"{case_id}: edge probe must have no saved advanced setting")

    callbacks = []
    callbacks_by_phase = {}
    phase_callback_counts = {}
    callback_fields = (
        "phase",
        "xy_bits",
        "previous_xy_bits",
        "stored_size_u32",
        "k_bits",
        "inverse_scale_bits",
        "pre_append_count_bytes",
        "vector_count_bytes",
    )
    for source in native["callback_add_point_calls"]:
        row = {key: source[key] for key in callback_fields}
        row["phase_callback_index"] = phase_callback_counts.get(source["phase"], 0)
        phase_callback_counts[source["phase"]] = row["phase_callback_index"] + 1
        callbacks.append(row)
        callbacks_by_phase.setdefault(source["phase"], []).append(row)

    transitions = []
    phase_transition_indices = {}
    for source in native_calls["v1_transition_boundaries"]:
        phase = source["phase"]
        index = phase_transition_indices.get(phase, 0)
        if source["phase_transition_index"] != index:
            raise RuntimeError(f"{case_id}/{phase}: native transition ordering changed")
        phase_transition_indices[phase] = index + 1
        row = {
            "phase": phase,
            "phase_transition_index": index,
            "method": source["method"],
            "signature": source["signature"],
            "callback_index_start": source["callback_index_start"],
            "callback_index_end": source["callback_index_end"],
            "receiver_before_80_176": source["receiver_before"]["receiver_80_176"],
            "receiver_after_80_176": source["receiver_after"]["receiver_80_176"],
            "rt_before_344_360": source["receiver_before"]["rt_344_360"],
            "rt_after_344_360": source["receiver_after"]["rt_344_360"],
            "vector_count_before_bytes": source["receiver_before"]["vector_count_bytes"],
            "vector_count_after_bytes": source["receiver_after"]["vector_count_bytes"],
            "args_s0_s3": source["float_args_s0_s3"],
        }
        if "motion_event_entry_bytes" in source:
            row["motion_event_entry_bytes"] = source["motion_event_entry_bytes"]
        if row["callback_index_end"] > phase_callback_counts.get(phase, 0):
            raise RuntimeError(f"{case_id}/{phase}: transition callback index exceeds phase stream")
        phase_callbacks = callbacks_by_phase.get(phase, [])
        expected_count = phase_callbacks[row["callback_index_end"] - 1]["vector_count_bytes"] if row["callback_index_end"] else 0
        if row["vector_count_after_bytes"] != expected_count:
            raise RuntimeError(f"{case_id}/{phase}: native vector bytes disagree with callback boundary")
        transitions.append(row)

    final_vectors = {}
    for phase, source in native["phase_final_vectors"].items():
        raw = bytes.fromhex(source["vector_bytes"])
        if len(raw) != source["vector_count_bytes"] or hashlib.sha256(raw).hexdigest() != source["sha256"]:
            raise RuntimeError(f"{case_id}/{phase}: native final vector length/hash mismatch")
        packed = base64.b64encode(zlib.compress(raw, level=9)).decode("ascii")
        decoded = zlib.decompress(base64.b64decode(packed, validate=True))
        if decoded != raw:
            raise RuntimeError(f"{case_id}/{phase}: compressed final vector round-trip mismatch")
        phase_rows = [row for row in transitions if row["phase"] == phase]
        if not phase_rows or phase_rows[-1]["rt_after_344_360"] is None:
            raise RuntimeError(f"{case_id}/{phase}: missing terminal RT snapshot")
        final_vectors[phase] = {
            "codec": "zlib+base64",
            "raw_byte_count": len(raw),
            "sha256": source["sha256"],
            "data": packed,
            "rt_344_360": phase_rows[-1]["rt_after_344_360"],
        }

    if set(phase_callback_counts) != {"draw_object_stroke", "saved_redraw"}:
        raise RuntimeError(f"{case_id}: missing native callback phase")
    for phase in phase_callback_counts:
        phase_rows = [row for row in transitions if row["phase"] == phase]
        if not phase_rows or phase_rows[-1]["method"] != "endPen" or phase_rows[-1]["callback_index_end"] != phase_callback_counts[phase]:
            raise RuntimeError(f"{case_id}/{phase}: endPen did not close the full callback stream")

    return {
        "case_id": case_id,
        "input": native["input"],
        "profile": native["profile"],
        "runtime": {
            "route": native_calls["route"],
            "source_ObjectStroke_arrays": native_calls["source_ObjectStroke_arrays"],
            "source_ObjectStroke_GetPenSize_f32_bits": native_calls["source_ObjectStroke_GetPenSize_f32_bits"],
            "ObjectStroke_getter_snapshots": native_calls["ObjectStroke_getter_snapshots"],
            "sampler_repeats": native_calls.get("sampler_repeats", []),
            "screen_resolution": native_calls["pen_screen_resolution"],
            "host_boundaries": native["host_boundaries"],
            "scope": native["scope"],
        },
        "libraries": native["libraries"],
        "callback_add_point_calls": callbacks,
        "transitions": transitions,
        "phase_final_vectors": final_vectors,
    }


def control_rows() -> dict:
    selected = json.loads((ROOT / "conformance/oblique-saved-caller-selected-native.json").read_text())
    stress = json.loads((ROOT / "conformance/oblique-v1-stress-native.json").read_text())
    return {
        "tool0": {
            "fixture": "conformance/oblique-saved-caller-selected-native.json",
            "case": "four_points_curve",
            "input_points_f32_bits": selected["cases"]["four_points_curve"]["input"]["points_f32_bits"],
            "size_f32_bits": selected["cases"]["four_points_curve"]["input"]["pen_size_f32_bits"],
            "mesh_sha256": selected["cases"]["four_points_curve"]["phase_final_vectors"]["saved_redraw"]["sha256"],
        },
        "tool2": {
            "fixture": "conformance/oblique-v1-stress-native.json",
            "case": "stress_tool2",
            "input_points_f32_bits": stress["cases"]["stress_tool2"]["input"]["points_f32_bits"],
            "size_f32_bits": stress["cases"]["stress_tool2"]["input"]["pen_size_f32_bits"],
            "mesh_sha256": stress["cases"]["stress_tool2"]["phase_final_vectors"]["saved_redraw"]["sha256"],
        },
    }


def result_for(selected: tuple[str, ...]) -> dict:
    cases = {case: compact_case(case) for case in selected}
    return {
        "schema": "sdocx-oblique-v1-edge-native-v1",
        "tool_controls": control_rows(),
        "cases": cases,
    }


def main() -> None:
    selected = (args.case,) if args.case else CASES
    if args.write_fixture:
        result = result_for(selected)
        if args.case and FIXTURE.exists():
            current = json.loads(FIXTURE.read_text())
            cases = current.get("cases", {})
            cases.update(result["cases"])
            result["cases"] = cases
        FIXTURE.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)} cases {', '.join(selected)}")
        return

    fixture = json.loads(FIXTURE.read_text())
    if fixture.get("schema") != "sdocx-oblique-v1-edge-native-v1":
        raise SystemExit("unexpected Oblique V1 edge fixture schema")
    if args.case is None and set(fixture.get("cases", {})) != set(CASES):
        raise SystemExit("Oblique V1 edge fixture case set changed")
    if fixture.get("tool_controls") != control_rows():
        raise SystemExit("Oblique V1 tool control fixture reference changed")
    for case in selected:
        actual = compact_case(case)
        if fixture.get("cases", {}).get(case) != actual:
            raise SystemExit(f"native Oblique V1 edge fixture mismatch for {case}")
        print(f"verified {case}")


if __name__ == "__main__":
    main()
