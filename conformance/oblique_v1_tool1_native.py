"""Capture selected-V1 Oblique tool-type threshold branches and meshes."""
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
FIXTURE = ROOT / "conformance/oblique-v1-tool1-native.json"
CASES = (
    "tool1_linear_same_mesh",
    "tool1_curve_duplicates_sub5",
    "tool1_curve_alternating_short_long",
    "tool1_curve_49_999",
    "tool1_curve_50_000",
    "tool1_curve_50_001",
    "tool1_linear_short_long",
    "tool3_curve_short_long",
    "tool3_linear_same_mesh",
)

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group(required=True)
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
parser.add_argument("--case", choices=CASES, help="write or check one tool-threshold case")
args = parser.parse_args()


def capture(case_id: str) -> dict:
    env = os.environ.copy()
    prior = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = str(ROOT / "scratch/apk-analysis-runtime/python") + (os.pathsep + prior if prior else "")
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
    calls = native["native_calls"]
    if calls["route"]["drawing_selected_drawable_version"] != "1":
        raise RuntimeError(f"{case_id}: selected caller did not choose V1")
    if native["input"]["advanced_setting"] is not None:
        raise RuntimeError(f"{case_id}: expected the selected default V1 route")

    expected_tool = int(case_id.split("_", 1)[0][4:])
    source_tool = native["input"]["tool_type"]
    if source_tool != expected_tool:
        raise RuntimeError(f"{case_id}: source tool type {source_tool} != expected {expected_tool}")

    callbacks = []
    callback_indices = {}
    for source in native["callback_add_point_calls"]:
        phase = source["phase"]
        index = callback_indices.get(phase, 0)
        callback_indices[phase] = index + 1
        callbacks.append({
            "phase": phase,
            "phase_callback_index": index,
            "xy_bits": source["xy_bits"],
            "previous_xy_bits": source["previous_xy_bits"],
            "stored_size_u32": source["stored_size_u32"],
            "k_bits": source["k_bits"],
            "inverse_scale_bits": source["inverse_scale_bits"],
            "pre_append_count_bytes": source["pre_append_count_bytes"],
            "vector_count_bytes": source["vector_count_bytes"],
        })
    callbacks_by_phase = {
        phase: [row for row in callbacks if row["phase"] == phase]
        for phase in ("draw_object_stroke", "saved_redraw")
    }
    if any(not rows for rows in callbacks_by_phase.values()):
        raise RuntimeError(f"{case_id}: missing native callback phase")

    transitions = []
    next_transition = {"draw_object_stroke": 0, "saved_redraw": 0}
    for source in calls["v1_transition_boundaries"]:
        phase = source["phase"]
        index = next_transition[phase]
        if source["phase_transition_index"] != index:
            raise RuntimeError(f"{case_id}/{phase}: transition order changed")
        next_transition[phase] += 1
        end = source["callback_index_end"]
        if end > len(callbacks_by_phase[phase]):
            raise RuntimeError(f"{case_id}/{phase}: transition exceeds callback stream")
        expected_bytes = callbacks_by_phase[phase][end - 1]["vector_count_bytes"] if end else 0
        if source["receiver_after"]["vector_count_bytes"] != expected_bytes:
            raise RuntimeError(f"{case_id}/{phase}: transition RT count mismatch")
        transitions.append({
            "phase": phase,
            "phase_transition_index": index,
            "method": source["method"],
            "signature": source["signature"],
            "callback_index_start": source["callback_index_start"],
            "callback_index_end": end,
            "receiver_before_80_176": source["receiver_before"]["receiver_80_176"],
            "receiver_after_80_176": source["receiver_after"]["receiver_80_176"],
            "rt_before_344_360": source["receiver_before"]["rt_344_360"],
            "rt_after_344_360": source["receiver_after"]["rt_344_360"],
            "vector_count_before_bytes": source["receiver_before"]["vector_count_bytes"],
            "vector_count_after_bytes": source["receiver_after"]["vector_count_bytes"],
            "args_s0_s3": source["float_args_s0_s3"],
        })

    dispatch = calls["v1_tool_dispatch_sites"]
    threshold_sites = calls["v1_redrawline_threshold_sites"]
    axis_sites = calls["v1_redrawline_axis_gate_sites"]
    for phase in callbacks_by_phase:
        rows = [row for row in dispatch if row["phase"] == phase]
        getter = next((row for row in rows if row["site"].endswith("GetToolType return")), None)
        selected = next((row for row in rows if row["site"].endswith("before receiver+96 store")), None)
        stored = next((row for row in rows if row["site"].endswith("after threshold store")), None)
        if getter is None or selected is None or stored is None:
            raise RuntimeError(f"{case_id}/{phase}: missing source-tool-to-threshold trace")
        if getter["tool_type_return_w0"] != expected_tool:
            raise RuntimeError(f"{case_id}/{phase}: native GetToolType return mismatch")
        if selected["selected_threshold_f32_bits"] != stored["receiver_threshold_f32_bits"]:
            raise RuntimeError(f"{case_id}/{phase}: selected threshold store mismatch")
        expected_threshold = "00004842" if expected_tool == 1 else "0000a040"
        if selected["selected_threshold_f32_bits"] != expected_threshold:
            raise RuntimeError(f"{case_id}/{phase}: unexpected selected threshold")
        compares = [row for row in threshold_sites if row["phase"] == phase]
        axes = [row for row in axis_sites if row["phase"] == phase]
        is_curve = native["input"]["curve_enabled"]
        redraw_calls = sum(row["method"] == "redrawLine" and row["phase"] == phase for row in transitions)
        linear_calls = sum(row["method"] == "redrawLineNoCurve" and row["phase"] == phase for row in transitions)
        axis_early_outs = sum(row["both_axis_gate_taken"] for row in axes)
        if is_curve and len(compares) + axis_early_outs != redraw_calls:
            raise RuntimeError(f"{case_id}/{phase}: redrawLine axis/length branch captures incomplete")
        if not is_curve and (compares or axes or redraw_calls or not linear_calls):
            raise RuntimeError(f"{case_id}/{phase}: linear path unexpectedly hit curve branch")

    final_vectors = {}
    for phase, source in native["phase_final_vectors"].items():
        raw = bytes.fromhex(source["vector_bytes"])
        if len(raw) != source["vector_count_bytes"] or hashlib.sha256(raw).hexdigest() != source["sha256"]:
            raise RuntimeError(f"{case_id}/{phase}: native vector length/hash mismatch")
        packed = base64.b64encode(zlib.compress(raw, 9)).decode("ascii")
        if zlib.decompress(base64.b64decode(packed, validate=True)) != raw:
            raise RuntimeError(f"{case_id}/{phase}: compressed mesh roundtrip failed")
        phase_transitions = [row for row in transitions if row["phase"] == phase]
        if not phase_transitions or phase_transitions[-1]["method"] != "endPen":
            raise RuntimeError(f"{case_id}/{phase}: missing terminal endPen boundary")
        if phase_transitions[-1]["callback_index_end"] != len(callbacks_by_phase[phase]):
            raise RuntimeError(f"{case_id}/{phase}: endPen did not close callback stream")
        final_vectors[phase] = {
            "codec": "zlib+base64",
            "raw_byte_count": len(raw),
            "sha256": source["sha256"],
            "data": packed,
            "rt_344_360": phase_transitions[-1]["rt_after_344_360"],
        }

    return {
        "case_id": case_id,
        "input": native["input"],
        "profile": native["profile"],
        "source_runtime": {
            "route": calls["route"],
            "source_ObjectStroke_arrays": calls["source_ObjectStroke_arrays"],
            "ObjectStroke_getter_snapshots": calls["ObjectStroke_getter_snapshots"],
            "getToolType_threshold_sites": dispatch,
            "redrawLine_threshold_branches": threshold_sites,
            "redrawLine_axis_gate_branches": axis_sites,
            "sampler_repeats": calls.get("sampler_repeats", []),
            "host_boundaries": native["host_boundaries"],
            "scope": native["scope"],
        },
        "libraries": native["libraries"],
        "callback_add_point_calls": callbacks,
        "transitions": transitions,
        "phase_final_vectors": final_vectors,
    }


def build(selected: tuple[str, ...]) -> dict:
    return {
        "schema": "sdocx-oblique-v1-tool1-threshold-native-v1",
        "branch_provenance": {
            "library": "scratch/apk-analysis-native/arm64-v8a/libSPenObliquePen.so",
            "redraw_objectstroke": "SPen::ObliquePenStrokeDrawableGLV1::redraw(SPen::ObjectStroke const*, SPen::RectF*, bool), offset 0x393b0",
            "tool_getter_return": "offset 0x39454, immediately after native ObjectStroke::GetToolType() call",
            "threshold_select_store": "offsets 0x39468 fcsel and 0x3946c str s0,[x21,#96]; tool==1 selects 50.0, otherwise 5.0",
            "segment_gate": "redrawLine offsets 0x38c3c..0x38c44 apply the 1.5 both-axis gate; offsets 0x38c6c..0x38c70 compare distance against [receiver,#96]; byte+89 toggles for a short segment",
            "MotionEvent_note": "The saved selected-ObjectStroke caller takes redraw(ObjectStroke); the separate MotionEvent overload branch is not this route.",
        },
        "cases": {case: capture(case) for case in selected},
    }


def main() -> None:
    selected = (args.case,) if args.case else CASES
    if args.write_fixture:
        result = build(selected)
        if args.case and FIXTURE.exists():
            current = json.loads(FIXTURE.read_text())
            if current.get("schema") != result["schema"]:
                raise SystemExit("unexpected existing tool1 fixture schema")
            current["cases"].update(result["cases"])
            result = current
        FIXTURE.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)} cases {', '.join(selected)}")
        return

    fixture = json.loads(FIXTURE.read_text())
    if fixture.get("schema") != "sdocx-oblique-v1-tool1-threshold-native-v1":
        raise SystemExit("unexpected Oblique V1 tool1 fixture schema")
    if fixture.get("branch_provenance") != build_provenance():
        raise SystemExit("tool1 branch provenance changed")
    if args.case is None and set(fixture.get("cases", {})) != set(CASES):
        raise SystemExit("Oblique V1 tool1 fixture case set changed")
    for case in selected:
        actual = capture(case)
        if fixture.get("cases", {}).get(case) != actual:
            raise SystemExit(f"tool1 fixture mismatch for {case}")
    print(f"verified {len(selected)} Oblique V1 tool cases")


def build_provenance() -> dict:
    return {
        "library": "scratch/apk-analysis-native/arm64-v8a/libSPenObliquePen.so",
        "redraw_objectstroke": "SPen::ObliquePenStrokeDrawableGLV1::redraw(SPen::ObjectStroke const*, SPen::RectF*, bool), offset 0x393b0",
        "tool_getter_return": "offset 0x39454, immediately after native ObjectStroke::GetToolType() call",
        "threshold_select_store": "offsets 0x39468 fcsel and 0x3946c str s0,[x21,#96]; tool==1 selects 50.0, otherwise 5.0",
        "segment_gate": "redrawLine offsets 0x38c3c..0x38c44 apply the 1.5 both-axis gate; offsets 0x38c6c..0x38c70 compare distance against [receiver,#96]; byte+89 toggles for a short segment",
        "MotionEvent_note": "The saved selected-ObjectStroke caller takes redraw(ObjectStroke); the separate MotionEvent overload branch is not this route.",
    }


if __name__ == "__main__":
    main()
