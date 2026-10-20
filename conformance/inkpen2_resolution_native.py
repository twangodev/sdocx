#!/usr/bin/env python3
"""Native InkPen2 V9 screen-dimension normalization controls.

Runs the existing pinned saved caller with four explicit host display profiles.
The child executes native Drawing selection, V9 redraw and AddPoint callbacks;
this wrapper checks the complete native RTV1 vectors and the screen context
contract without modifying the source caller.

From the repository root, using the project-local Unicorn runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 \
    conformance/inkpen2_resolution_native.py --check-fixture
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/inkpen2-resolution-native.json"
CALLER = ROOT / "conformance/inkpen2_saved_caller_native.py"
RUNTIME_PY = ROOT / "scratch/apk-analysis-runtime/python"
BINUTILS = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"
RUNTIME_LIB = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
APK_LIBS = ROOT / "scratch/apk-analysis-native/arm64-v8a"

SOURCE_PINS = {
    "conformance/inkpen2_saved_caller_native.py": "a1d0f06b88f5de50274f81228f04020231a7d23089581fbee99252648a3676aa",
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
}
LIBRARY_PINS = {
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenPenCommon.so": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libSPenDrawing.so": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "libSPenGraphics.so": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "libSPenInkPen2.so": "61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c",
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenFountainPen.so": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}
RUNTIME_FILE_PINS = {
    "unicorn/__init__.py": "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b",
    "unicorn/arm64_const.py": "92f91a2b0a36cd34deee16db4f8da4ac0c9ae35c3ea1089296e5ef9d170a5be9",
    "unicorn/unicorn.py": "e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7",
    "unicorn/unicorn_py3/unicorn.py": "62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0",
    "unicorn/lib/libunicorn.so.2": "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c",
}
TOOL_PINS = {
    "/usr/bin/x86_64-linux-gnu-nm": "ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1",
    "/usr/bin/x86_64-linux-gnu-c++filt": "7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2",
    "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump": "b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb",
}
PYTHON_PIN = "889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9"

PROFILES = {
    "portrait_1920x1080": (1920, 1080),
    "landscape_1080x1920": (1080, 1920),
    "square_1080": (1080, 1080),
    "square_1920": (1920, 1920),
}
CASE = "line0_curve_short_history"
ADVANCED_SETTING = "9"
NATIVE_INSTRUCTIONS = {
    "drawing_context_and_pen_setter": {
        "library": "libSPenDrawing.so",
        "rva_range": ["0x82390", "0x823f8"],
        "expected": {
            "0x82398": "f9400908", "0x8239c": "d63f0100",
            "0x823b8": "f9401108", "0x823bc": "d63f0100",
            "0x823d8": "f9401508", "0x823dc": "d63f0100",
            "0x823e8": "f9403908", "0x823ec": "aa1503e0",
            "0x823f0": "2a1603e1", "0x823f4": "d63f0100",
        },
        "meaning": "context+16 returns screen-profile object; its slots+32/+40 return width/height; selected Pen slot+112 receives x0=pen,w1=width,w2=height",
    },
    "pen_set_screen_resolution": {
        "library": "libSPenPenCommon.so",
        "rva_range": ["0x460b4", "0x460c8"],
        "expected": {
            "0x460b4": "aa0003e8", "0x460b8": "b9003801",
            "0x460bc": "52800020", "0x460c0": "b9003d02",
            "0x460c4": "d65f03c0",
        },
        "meaning": "native SetScreenResolution stores W1 at Pen+56 and W2 at Pen+60, then returns 1",
    },
    "drawline_two_dimension_min": {
        "library": "libSPenInkPen2.so",
        "rva_range": ["0x4de38", "0x4de88"],
        "expected": {
            "0x4de38": "f9402668", "0x4de3c": "f9400108",
            "0x4de44": "2944250a", "0x4de4c": "6b0a013f",
            "0x4de50": "1a8ab129", "0x4de64": "1e220122",
            "0x4de68": "52a89689", "0x4de6c": "1e270123",
            "0x4de70": "1e221862", "0x4de84": "1e220821",
        },
        "meaning": "load two dimensions at data+32/+36; compare and signed-csel the smaller; calculate 1440.0/min and apply it to the time-derived factor",
    },
}


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_hash(path: pathlib.Path, expected: str, label: str) -> dict:
    if not path.is_file():
        raise RuntimeError(f"missing pinned {label}: {path}")
    actual = sha256(path)
    if actual != expected:
        raise RuntimeError(f"{label} hash mismatch: {path}: {actual} != {expected}")
    return {"path": str(path), "sha256": actual}


def preflight() -> dict:
    files = {}
    for rel, expected in SOURCE_PINS.items():
        files[rel] = check_hash(ROOT / rel, expected, rel)
    for name, expected in LIBRARY_PINS.items():
        files[f"native/{name}"] = check_hash(APK_LIBS / name, expected, name)
    for rel, expected in RUNTIME_FILE_PINS.items():
        files[f"runtime/{rel}"] = check_hash(RUNTIME_PY / rel, expected, rel)
    python = pathlib.Path(sys.executable).resolve()
    files["python"] = check_hash(python, PYTHON_PIN, "Python executable")
    tools = {}
    for rel, expected in TOOL_PINS.items():
        path = (ROOT / rel).resolve() if not rel.startswith("/") else pathlib.Path(rel).resolve()
        tool = check_hash(path, expected, rel)
        tools[rel] = tool
    for name in ("nm", "c++filt"):
        resolved = pathlib.Path(shutil.which(name) or "").resolve()
        if str(resolved) not in TOOL_PINS:
            raise RuntimeError(f"{name} resolved outside the pinned tool set: {resolved}")
        expected = TOOL_PINS[str(resolved)]
        tools[f"resolved:{name}"] = check_hash(resolved, expected, name)
    return {
        "python_version": platform.python_version(),
        "python_executable": str(python),
        "files": files,
        "tools": tools,
        "case": CASE,
        "advanced_setting": ADVANCED_SETTING,
    }


def instruction_capture() -> dict:
    objdump = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = str(RUNTIME_LIB.resolve())
    result = {}
    for key, spec in NATIVE_INSTRUCTIONS.items():
        path = APK_LIBS / spec["library"]
        output = subprocess.check_output([
            str(objdump), "-d", "-C", f"--start-address={spec['rva_range'][0]}",
            f"--stop-address={spec['rva_range'][1]}", str(path),
        ], cwd=ROOT, env=env, text=True)
        found = {
            f"0x{int(address, 16):x}": {"bytes": raw.lower(), "assembly": assembly.strip()}
            for address, raw, assembly in re.findall(
                r"^\s*([0-9a-f]+):\s+([0-9a-f]{8})\s+(.+)$", output, re.M
            )
        }
        actual = {address: found.get(address, {}).get("bytes") for address in spec["expected"]}
        if actual != spec["expected"]:
            raise RuntimeError(f"pinned native instruction bytes differ for {key}: {actual}")
        result[key] = {"library": spec["library"], "rva_range": spec["rva_range"],
                       "instructions": {address: {"bytes": raw, "assembly": found[address]["assembly"]}
                                        for address, raw in spec["expected"].items()},
                       "meaning": spec["meaning"]}
    return result


def capture_profile(profile_id: str, width: int, height: int) -> dict:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(RUNTIME_PY.resolve()) + os.pathsep + env.get("PYTHONPATH", "")
    env["LD_LIBRARY_PATH"] = str(RUNTIME_LIB.resolve())
    command = [sys.executable, str(CALLER), "--print-capture", "--case", CASE,
               "--advanced-setting", ADVANCED_SETTING, "--width", str(width), "--height", str(height)]
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if completed.returncode:
        raise RuntimeError(f"native caller failed for {profile_id}: {completed.stderr[-2500:]}")
    native = json.loads(completed.stdout)
    calls = native["native_calls"]
    dimensions = calls["pen_screen_resolution"]
    if dimensions != [{"width": width, "height": height}]:
        raise RuntimeError(f"native SetScreenResolution args differ for {profile_id}: {dimensions}")
    if native["profile"]["width"] != width or native["profile"]["height"] != height:
        raise RuntimeError(f"native caller profile mismatch for {profile_id}")
    if calls["ObjectDrawing::drawObjectStroke_return"] != 1 or calls["selected_drawable_object_redraw_return"] != 1:
        raise RuntimeError(f"native saved caller failed for {profile_id}")
    if calls["selected_drawable_primary_vtable_id"] != "InkPen2StrokeDrawableGLV9":
        raise RuntimeError(f"unexpected drawable for {profile_id}")
    phase_streams = {}
    for phase in ("draw_object_stroke", "saved_redraw"):
        events = [row for row in native["callback_buffer_events"] if row["phase"] == phase]
        if not events:
            raise RuntimeError(f"missing native AddPoint stream in {phase} for {profile_id}")
        raw = bytes.fromhex(events[-1]["after_bytes"])
        if len(raw) != 900 or len(raw) % 36:
            raise RuntimeError(f"unexpected full RTV1 vector size in {phase}: {len(raw)}")
        phase_streams[phase] = {"bytes_hex": raw.hex(), "length": len(raw),
                                "record_count": len(raw) // 36,
                                "sha256": hashlib.sha256(raw).hexdigest()}
    if phase_streams["draw_object_stroke"] != phase_streams["saved_redraw"]:
        raise RuntimeError(f"Drawing callback differs from selected saved RedrawPen replay for {profile_id}")
    return {
        "profile_id": profile_id,
        "host_context_getter": {
            "object_drawing_context_vslot": 16,
            "screen_profile_getter_slots": {"width": 32, "height": 40},
            "returned_dimensions": [width, height],
            "source": "explicit synthetic host profile; vslot callbacks in pinned caller",
        },
        "native_calls": {
            "ObjectDrawing_drawObjectStroke_return": calls["ObjectDrawing::drawObjectStroke_return"],
            "selected_drawable": calls["selected_drawable_primary_vtable_id"],
            "Pen_SetScreenResolution_entry_args": dimensions[0],
            "selected_RedrawPen_return": calls["selected_drawable_object_redraw_return"],
            "source_profile": native["profile"],
        },
        "native_rtv1_streams": phase_streams,
        "source": native["input"],
        "scope": "native V9 Drawing saved-object caller under explicit synthetic context; host provides display getter and bounded resource/queue callbacks",
    }


def produce() -> dict:
    provenance = preflight()
    native_instructions = instruction_capture()
    cases = {case_id: capture_profile(case_id, *profile) for case_id, profile in PROFILES.items()}
    reference = cases["portrait_1920x1080"]["native_rtv1_streams"]["draw_object_stroke"]
    for name in ("landscape_1080x1920", "square_1080"):
        if cases[name]["native_rtv1_streams"]["draw_object_stroke"] != reference:
            raise RuntimeError(f"min-dimension orientation control differs: {name}")
    square_1920 = cases["square_1920"]["native_rtv1_streams"]["draw_object_stroke"]
    if square_1920["sha256"] == reference["sha256"] or square_1920["bytes_hex"] == reference["bytes_hex"]:
        raise RuntimeError("1920-square control did not change the complete native stream")
    return {
        "schema": "inkpen2-native-screen-normalization-v1",
        "provenance": provenance,
        "native_instruction_evidence": native_instructions,
        "profiles": cases,
        "assertions": {
            "orientation_swap_same_minimum_same_full_stream": ["portrait_1920x1080", "landscape_1080x1920"],
            "square_1080_same_minimum_same_full_stream": ["portrait_1920x1080", "square_1080"],
            "square_1920_changes_full_stream": ["portrait_1920x1080", "square_1920"],
            "native_vector_format": "25 records x 36 bytes = 900 bytes per captured AddPoint vector",
        },
        "limits": [
            "all profiles are explicit synthetic context getter outputs; the fixture does not identify Samsung's ordinary export/display profile",
            "the source caller executes native Drawing selection, selected V9 redraw and AddPoint; bounded host callbacks provide context, resources and task queue",
            "the stream is a native geometry/vector oracle, not a renderer pixel or PDF/SVG export claim",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    if not args.write_fixture and not args.check_fixture:
        args.check_fixture = True
    existing = json.loads(FIXTURE.read_text()) if FIXTURE.exists() else None
    if args.check_fixture:
        if existing is None:
            raise SystemExit(f"missing fixture: {FIXTURE}")
        current_runner = sha256(pathlib.Path(__file__).resolve())
        if existing.get("runner_sha256") != current_runner:
            raise SystemExit("runner SHA-256 differs from frozen fixture")
    result = produce()
    result["runner_sha256"] = sha256(pathlib.Path(__file__).resolve())
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif json.loads(encoded) != existing:
        raise SystemExit("fresh native resolution capture differs from frozen fixture")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
