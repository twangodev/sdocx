"""Compact native stress oracle for the selected Oblique V1 saved redraw path."""
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
FIXTURE = ROOT / "conformance/oblique-v1-stress-native.json"
CASES = (
    "stress_six_curve",
    "stress_six_linear",
    "stress_gate_alternation",
    "stress_tool2",
    "stress_size4",
    "stress_size8",
    "stress_size16",
    "stress_size56",
)

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group(required=True)
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
parser.add_argument("--case", choices=CASES, help="check or write one stress case")
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
        expected_vector_count = phase_callbacks[row["callback_index_end"] - 1]["vector_count_bytes"] if row["callback_index_end"] else 0
        if row["vector_count_after_bytes"] != expected_vector_count:
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
        rows = [row for row in transitions if row["phase"] == phase]
        if not rows or rows[-1]["method"] != "endPen" or rows[-1]["callback_index_end"] != phase_callback_counts[phase]:
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


def main() -> None:
    selected = (args.case,) if args.case else CASES
    if args.write_fixture:
        result = {case: compact_case(case) for case in selected}
        if args.case and FIXTURE.exists():
            existing = json.loads(FIXTURE.read_text())
            cases = existing.get("cases", {})
            cases.update(result)
        else:
            cases = result
        FIXTURE.write_text(json.dumps({"schema": "sdocx-oblique-v1-stress-native-v1", "cases": cases}, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)} cases {', '.join(selected)}")
        return

    fixture = json.loads(FIXTURE.read_text())
    if fixture.get("schema") != "sdocx-oblique-v1-stress-native-v1":
        raise SystemExit("unexpected Oblique V1 stress fixture schema")
    if args.case is None and set(fixture.get("cases", {})) != set(CASES):
        raise SystemExit("Oblique V1 stress fixture case set changed")
    for case in selected:
        actual = compact_case(case)
        expected = fixture.get("cases", {}).get(case)
        if expected != actual:
            raise SystemExit(f"native Oblique V1 stress fixture mismatch for {case}")
        print(f"verified {FIXTURE.relative_to(ROOT)} case {case}")


if __name__ == "__main__":
    main()
