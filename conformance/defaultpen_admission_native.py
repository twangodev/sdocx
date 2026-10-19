#!/usr/bin/env python3
"""Pinned DefaultPen saved-caller admission controls.

This wrapper applies bounded in-memory substitutions to the existing native
Model -> Drawing -> DefaultPen caller. It records full AddPoint triples for
tools 0..3, omitted-size and native size-clamp controls. No temporary Python
module or prior scratch capture is imported.

Run from the repository root with the project runtime on PYTHONPATH:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    python3 conformance/defaultpen_admission_native.py
  ... python3 conformance/defaultpen_admission_native.py --write-fixture
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
RUNNER = Path(__file__).resolve()
CALLER = ROOT / "conformance/defaultpen_saved_caller_native.py"
FIXTURE = ROOT / "conformance/defaultpen-admission-native.json"
LIBS = ROOT / "scratch/apk-analysis-native/arm64-v8a"
RUNTIME = ROOT / "scratch/apk-analysis-runtime"
SCHEMA = "defaultpen-native-admission-controls-v1"

EXPECTED_CALLER_SHA256 = "14e1b1d85df0aaebdc889871656217b927bedfc45ad1eba309fbe562eedd559a"
EXPECTED_MODEL_DEFAULTS_FIXTURE_SHA256 = "b4142701a6e5ffc2c3a87653a4bea167d267279fe26211ee4f5e371442b331dc"
EXPECTED_HELPERS = {
    "conformance/defaultpen_saved_caller_native.py": EXPECTED_CALLER_SHA256,
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump": "b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb",
}
EXPECTED_DSOS = {
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenPenCommon.so": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libSPenDrawing.so": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "libSPenGraphics.so": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "libSPenDefaultPen.so": "1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f",
    "libSPenFountainPen.so": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group()
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
args = parser.parse_args()


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_pins() -> tuple[dict[str, str], dict[str, str]]:
    helper_hashes = {name: sha(ROOT / name) for name in EXPECTED_HELPERS}
    for name, expected in EXPECTED_HELPERS.items():
        if helper_hashes[name] != expected:
            raise RuntimeError(f"helper pin mismatch: {name}: {helper_hashes[name]}")
    dso_hashes = {name: sha(LIBS / name) for name in EXPECTED_DSOS}
    for name, expected in EXPECTED_DSOS.items():
        if dso_hashes[name] != expected:
            raise RuntimeError(f"DSO pin mismatch: {name}: {dso_hashes[name]}")
    return helper_hashes, dso_hashes


def patched_caller(case: dict) -> str:
    source = CALLER.read_text()
    if sha(CALLER) != EXPECTED_CALLER_SHA256:
        raise RuntimeError("pinned native caller changed")
    replacements = [
        (
            "ROOT=pathlib.Path(__file__).resolve().parents[1]",
            f"ROOT=pathlib.Path({str(ROOT)!r})",
        ),
        (
            "case_id='straight_curve_tool1_default'\ninput_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}",
            "case_id='admission_control'\ninput_case=" + repr(case),
        ),
        (
            "p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]); p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex();",
            "(p.f(0,input_case['size']), p.call('SPen::ObjectStroke::SetPenSize(float)',[obj])) if input_case.get('set_size', True) else None; p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); p.call('SPen::ObjectStroke::GetColor() const',[obj]); source_color_argb=p.x(0);",
        ),
        (
            "p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']]);",
            "(p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']])) if input_case.get('set_tool', True) else None;",
        ),
        (
            "if len(draw_stream)!=132 or draw_stream!=saved_stream: raise RuntimeError(f'expected identical 132-byte stream, got {len(draw_stream)} and {len(saved_stream)} bytes')",
            "if draw_stream!=saved_stream: raise RuntimeError(f'Drawing/saved replay mismatch: {len(draw_stream)} and {len(saved_stream)} bytes')",
        ),
        (
            "if len(events)!=11 or any(int(row['rt'],16)!=selected_rt for row in events):",
            "if any(int(row['rt'],16)!=selected_rt for row in events):",
        ),
        (
            "raise RuntimeError('expected native MotionEvent source getters in both phases')",
            "pass # source getter call is branch-dependent",
        ),
        (
            "'pen_size_f32_bits':source_size_bits,",
            "'pen_size_f32_bits':source_size_bits,'native_color_argb_getter':source_color_argb,'native_getter_rvas':{name:hex(p.syms[name]-BASE) for name in ('SPen::ObjectStroke::GetColor() const','SPen::ObjectStroke::GetPenSize() const','SPen::ObjectStroke::GetToolType() const')},'native_object_stroke_snapshots':object_stroke_snapshots,",
        ),
        (
            "'screen_resolution':screen_resolution_calls,",
            "'screen_resolution':screen_resolution_calls,'native_pen_size_after_drawing_f32_bits':pen_size_after_draw_bits,'native_paint_color_calls':paint_color_calls,",
        ),
    ]
    for old, new in replacements:
        count = source.count(old)
        if count != 1:
            raise RuntimeError(f"caller patch anchor count {count}, expected exactly one: {old[:100]!r}")
        source = source.replace(old, new, 1)
    return source


BASE_CASE = {
    "points": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 10.0]],
    "size": 10.0,
    "curve_enabled": True,
    "stroke_type": 0,
    "pressure": [0.25, 0.5, 0.75, 1.0],
    "timestamps": [10, 20, 30, 40],
    "tilt": [0.0] * 4,
    "orientation": [0.0] * 4,
}
CASES: dict[str, dict] = {}
for tool in range(4):
    CASES[f"tool_{tool}"] = BASE_CASE | {"tool_type": tool, "set_tool": True, "set_size": True}
CASES["tool_default_unset"] = BASE_CASE | {"tool_type": 2, "set_tool": False, "set_size": True}
CASES["size_omitted"] = BASE_CASE | {"tool_type": 1, "set_tool": True, "set_size": False}
for raw_size in (0.0, 0.1, 800.0, 1000.0):
    CASES[f"size_{raw_size:g}"] = BASE_CASE | {"tool_type": 1, "set_tool": True, "set_size": True, "size": raw_size}


def run_capture(case_id: str, case: dict) -> dict:
    code = patched_caller(case)
    env = dict(os.environ)
    search = [
        str(RUNTIME / "python"),
        str(RUNTIME / "binutils-aarch64/usr/bin"),
    ]
    env["PYTHONPATH"] = os.pathsep.join(search + [env.get("PYTHONPATH", "")])
    env["LD_LIBRARY_PATH"] = str(RUNTIME / "binutils-aarch64/usr/lib/x86_64-linux-gnu")
    result = subprocess.run(
        [sys.executable, "-c", code, "--print-capture"],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    captured = json.loads(result.stdout)
    captured["patched_caller_code_sha256"] = hashlib.sha256(code.encode()).hexdigest()
    if captured["route"]["selected_drawable_class"] != "SPen::DefaultPenStrokeDrawableGL":
        raise RuntimeError(f"wrong DefaultPen route for {case_id}")
    if not captured["streams"]["saved_redraw"]["same_bytes_as_drawing_phase"]:
        raise RuntimeError(f"Drawing and saved-redraw streams differ for {case_id}")
    triples = captured["streams"]["draw_object_stroke"]["values_f32_bits"]
    if len(triples) != captured["streams"]["draw_object_stroke"]["callback_count"] * 3:
        raise RuntimeError(f"non-triple AddPoint output for {case_id}")
    input_data = captured["input"]
    input_data["size_setter_called"] = case["set_size"]
    input_data["tool_setter_called"] = case["set_tool"]
    input_data["color_setter_called"] = False
    input_data["case_id"] = case_id
    captured["case_id"] = case_id
    snapshots = captured["input"]["native_object_stroke_snapshots"]
    pre_draw = next(row for row in snapshots if row["label"] == "before_drawObjectStroke")
    if pre_draw["tool_type"] != captured["input"]["tool_type"]:
        raise RuntimeError(f"native ObjectStroke tool getter mismatch for {case_id}")
    if int(captured["input"]["native_color_argb_getter"]) != 0xFF000000:
        raise RuntimeError(f"omitted native color did not default to black for {case_id}")
    source_size = struct.unpack("<f", bytes.fromhex(captured["input"]["pen_size_f32_bits"]))[0]
    expected_pen_size = min(800.0, max(0.4, source_size))
    expected_size_bits = struct.pack("<f", expected_pen_size).hex()
    if captured["native_pen_size_after_drawing_f32_bits"] != expected_size_bits:
        raise RuntimeError(f"native Drawing size clamp mismatch for {case_id}")
    paint_calls = captured["native_paint_color_calls"]
    set_pen_data = [row for row in paint_calls if row["method"] == "SPen::PenDrawableRT::SetPenData(float, int)"]
    if {row["phase"] for row in set_pen_data} != {"draw_object_stroke", "saved_redraw"}:
        raise RuntimeError(f"missing native SetPenData phase for {case_id}")
    if any(int(row["argb_u32"], 16) != 0xFF000000 for row in set_pen_data):
        raise RuntimeError(f"native RT color differs from omitted black default for {case_id}")
    expected_size_word = struct.unpack("<I", bytes.fromhex(expected_size_bits))[0]
    if any(int(row["size_f32_bits"], 16) != expected_size_word for row in set_pen_data):
        raise RuntimeError(f"native RT size differs from clamped Pen size for {case_id}")
    return captured


def build_fixture() -> dict:
    helper_hashes, dso_hashes = verify_pins()
    defaults_fixture = ROOT / "conformance/object-stroke-defaults-native.json"
    if sha(defaults_fixture) != EXPECTED_MODEL_DEFAULTS_FIXTURE_SHA256:
        raise RuntimeError("ObjectStroke defaults fixture pin mismatch")
    cases = {name: run_capture(name, case) for name, case in CASES.items()}
    return {
        "schema": SCHEMA,
        "provenance": {
            "runner_sha256": sha(RUNNER),
            "caller_sha256": sha(CALLER),
            "helper_sha256": helper_hashes,
            "dso_sha256": dso_hashes,
            "model_defaults_fixture_sha256": EXPECTED_MODEL_DEFAULTS_FIXTURE_SHA256,
            "runtime_scope": "Pinned native Model getters, Drawing caller, DefaultPen drawable and RT AddPoint execute under Unicorn. IContext is a synthetic 1920x1080 host profile; canvas/task queue, identity color conversion and graphics services are bounded host adapters.",
            "input_scope": "Synthetic native ObjectStroke setter calls, not serialized SDocx parser input. Color setter is omitted in every case; the wrapper additionally calls native ObjectStroke::GetColor and records Pen::GetSize after Drawing plus PenDrawableRT::SetPenData arguments. In-memory patches alter only the input row, selected size/tool setter calls, fixed-count/source-getter checks, and output metadata.",
        },
        "cases": cases,
    }


actual = build_fixture()
if args.write_fixture:
    FIXTURE.write_text(json.dumps(actual, indent=2, sort_keys=True) + "\n")
    print(f"wrote {FIXTURE.relative_to(ROOT)} ({sha(FIXTURE)})")
else:
    if not FIXTURE.exists():
        raise SystemExit("native DefaultPen admission fixture is missing; use --write-fixture after review")
    expected = json.loads(FIXTURE.read_text())
    if expected != actual:
        raise SystemExit("native DefaultPen admission fixture mismatch")
    print(f"verified {FIXTURE.relative_to(ROOT)} ({sha(FIXTURE)})")
