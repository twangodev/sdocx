#!/usr/bin/env python3
"""Replay native InkPen V1 no-curve tool 3/4 sample and clamp boundaries.

Inputs are synthetic native ObjectStroke setters. The default command replays
and checks the fixture; writing requires the explicit --write-fixture flag.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/inkpen-v1-nocurve-tool34-native.json"
HARNESS = ROOT / "conformance/inkpen_v1_boundaries_native.py"
BASE_CASES = ROOT / "conformance/inkpen-v1-saved-native.json"
WRAPPER = ROOT / "conformance/inkpen_v1_reuse_native.py"
PROFILE = {"width": 1920, "height": 1080}
PINNED_INPUTS = {
    "conformance/inkpen_v1_boundaries_native.py": "dc2a2ef356eab6dd826673f6cf77ff2269ab74a080463f943ff7d2e97fad4aec",
    "conformance/inkpen-v1-saved-native.json": "ade2af7ee7c7c07d4663151074932009f7def1a91ba4c14a4c55c294a3e32fb4",
    "conformance/inkpen_v1_reuse_native.py": "8306643a8d83aa9ed7ce06984ccc2540cc4ce426bb73b3bd596e9b637c042006",
}
PINNED_CALLER_SIZE_GUARD = (
    "if setting_resolution != [args.width,args.height] or setting_size_bits != source_size_bits: "
    "raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')"
)
CLAMPED_CALLER_SIZE_GUARD = (
    "if setting_resolution != [args.width,args.height] or setting_size_bits != "
    "struct.pack('<f',min(max(struct.unpack('<f',bytes.fromhex(source_size_bits))[0],.4),800.)).hex(): "
    "raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')"
)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def f32_bits(value):
    return struct.pack("<f", value).hex()


def f32_from_bits(value):
    return struct.unpack("<f", bytes.fromhex(value))[0]


def make_row(points, tool, size=4.0, pressure=None):
    count = len(points)
    if pressure is None:
        pressure = [0.2 + 0.15 * i for i in range(count)]
    return {
        "points": points,
        "size": size,
        "curve_enabled": False,
        "tool_type": tool,
        "stroke_type": 0,
        "pressure": pressure,
        "timestamps": [100 + 17 * i for i in range(count)],
        "tilt": [0.0] * count,
        "orientation": [0.0] * count,
    }


POINTS3 = [(2.25, 1.5), (4.875, 3.25), (12.25, 10.75)]
POINTS5 = [(7.25, 3.5), (8.5, 10.0), (16.25, 12.75), (24.5, 4.25), (32.75, 18.5)]
POINTS6 = POINTS5 + [(39.5, 21.25)]


def case_inputs():
    cases = {
        "tool3_three_nonsymmetric": make_row(POINTS3, 3),
        "tool3_five_nonsymmetric": make_row(POINTS5, 3),
        "tool4_three_nonsymmetric": make_row(POINTS3, 4),
        "tool4_five_nonsymmetric": make_row(POINTS5, 4),
        "tool3_pressure_edges": make_row(POINTS6, 3, pressure=[0.2, -0.25, 0.0, 0.001, 1.0, 1.001]),
        "tool4_pressure_edges": make_row(POINTS6, 4, pressure=[0.2, -0.25, 0.0, 0.001, 1.0, 1.001]),
    }
    for size in (0.1, 0.4, 800.0, 1000.0):
        key = str(size).replace(".", "_")
        cases[f"tool3_size_{key}"] = make_row(POINTS3, 3, size=size, pressure=[0.25, 0.5, 1.0])
    return cases


def preflight():
    for relative, expected in PINNED_INPUTS.items():
        actual = sha(ROOT / relative)
        if actual != expected:
            raise RuntimeError(f"pinned conformance input changed: {relative}: {actual}")


def capture_cases():
    preflight()
    harness = load("inkpen_boundary_harness", HARNESS)
    wrapper = load("inkpen_v1_reuse_wrapper", WRAPPER)
    pins = wrapper.verify_pins()
    caller = harness.load("inkpen_v1_reuse_caller", wrapper.PINNED_RUNNER)
    patched = harness.add_no_curve_line_returns(
        harness.add_no_curve_trace(wrapper.instrument_v1_trace(caller))
    )
    if patched.count(PINNED_CALLER_SIZE_GUARD) != 1:
        raise RuntimeError("pinned caller Pen-size validation anchor changed")
    patched = patched.replace(PINNED_CALLER_SIZE_GUARD, CLAMPED_CALLER_SIZE_GUARD, 1)
    caller.patched_caller_source = lambda: patched

    output = {}
    for name, input_row in case_inputs().items():
        row = harness.capture(caller, input_row, name, PROFILE)
        if row["route"]["selected_drawable_vtable_id"] != "v1":
            raise RuntimeError(f"{name}: native route did not select V1")
        if row["input"]["curve_enabled"] or row["input"]["tool_type"] != input_row["tool_type"]:
            raise RuntimeError(f"{name}: native curve/tool getter differs from requested input")
        if row["input"]["stroke_type"] != 0:
            raise RuntimeError(f"{name}: unexpected native StrokeType")
        if row["input"]["pen_size_f32_bits"] != f32_bits(input_row["size"]):
            raise RuntimeError(f"{name}: native ObjectStroke GetPenSize differs from input")
        expected_size = min(max(input_row["size"], 0.4), 800.0)
        expected_size_bits = f32_bits(expected_size)
        if row["pen_setting_data"]["pen_size_f32_bits_offset_0"] != expected_size_bits:
            raise RuntimeError(f"{name}: native Pen setting size does not match SetSize clamp")
        if row["drawable_native_state"]["drawable_size_f32_bits"] != expected_size_bits:
            raise RuntimeError(f"{name}: native drawable size does not match Pen size")

        row["event_boundary_summary"] = harness.validate_event_ranges(row, input_row, name)
        for phase in ("draw_object_stroke", "saved_redraw"):
            segments = [event for event in row["reuse"]["native_no_curve_line_ranges"] if event["phase"] == phase]
            if len(segments) != len(input_row["points"]) - 1:
                raise RuntimeError(f"{name}/{phase}: source segment count differs")
            for index, event in enumerate(segments, 1):
                expected_pressure = min(input_row["pressure"][index], 1.0)
                if event["args_s0_s2_bits"][2] != hex(struct.unpack("<I", struct.pack("<f", expected_pressure))[0]):
                    raise RuntimeError(f"{name}/{phase}: native no-curve pressure argument differs at index {index}")
        output[name] = {
            "tool_type_raw": input_row["tool_type"],
            "curve_enabled": False,
            "screen_resolution": [PROFILE["width"], PROFILE["height"]],
            "stroke": {
                "pen_width": input_row["size"],
                "points": [{"x": x, "y": y} for x, y in input_row["points"]],
                "pressures": input_row["pressure"],
                "timestamps": input_row["timestamps"],
                "tilts": input_row["tilt"],
                "orientations": input_row["orientation"],
            },
            "native": row,
        }
        print(f"captured {name}: {row['streams']['draw_object_stroke']['callback_count']} records", flush=True)

    return {
        "schema": "sdocx-inkpen-v1-nocurve-tool34-native-fixture-v1",
        "scope": "Fresh synthetic native Model setter inputs through selected V1 Drawing and direct saved redraw. Includes tools 3/4 with curve disabled, three/six-point pressure controls, native no-curve per-segment return prefixes, terminal ranges, and full RTV1 triples. Does not claim decoded-note admission or renderer/export parity.",
        "provenance": {
            "runner_sha256": sha(Path(__file__).resolve()),
            "pinned_inputs": {relative: expected for relative, expected in PINNED_INPUTS.items()},
            "pinned_native_closure": pins,
            "in_memory_patched_caller_sha256": hashlib.sha256(patched.encode()).hexdigest(),
            "in_memory_clamp_validation_change": "replaces the pinned caller assertion that Pen size equals requested ObjectStroke size with the separately checked native SetSize clamp [0.4,800]; source getter bits remain asserted unchanged",
            "base_numeric_fixture_sha256": sha(BASE_CASES),
        },
        "native_contract": {
            "no_curve_segment_symbol": "SPen::InkPenStrokeDrawableGLV1::drawLine_NoCurve(float,float,float,RectF*)",
            "segment_entry_return_pc": ["0x5028328", "0x5027ad8"],
            "history_batch_pc": ["0x50279b8", "0x5027760"],
            "terminal_pc": ["0x5025db8", "0x5027b88"],
            "pressure_rule_under_test": "No-curve tools 3/4 forward raw per-point pressure, with values above 1 clamped to 1; negative values are preserved as the native S2 argument in the tested range.",
            "size_rule_under_test": "ObjectStroke GetPenSize reports requested size; native Pen and selected drawable size use SetSize clamp [0.4,800].",
            "boundary_rule": "sample_ends count the RT vector after each ordered drawLine_NoCurve return; endPen_NoCurve output is kept as an unassigned terminal range.",
        },
        "cases": output,
    }


def main():
    current = capture_cases()
    encoded = json.dumps(current, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(current['cases'])} cases)")
        return
    if not FIXTURE.exists():
        raise SystemExit("InkPen V1 no-curve tool3/4 fixture missing; use --write-fixture explicitly")
    if json.loads(FIXTURE.read_text()) != current:
        raise SystemExit("InkPen V1 no-curve tool3/4 native fixture mismatch")
    print(f"verified {len(current['cases'])} native no-curve tool3/4 cases")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true", help="explicitly write or refresh the fixture")
    mode.add_argument("--check-fixture", action="store_true", help="explicitly replay and compare the fixture")
    args = parser.parse_args()
    main()
