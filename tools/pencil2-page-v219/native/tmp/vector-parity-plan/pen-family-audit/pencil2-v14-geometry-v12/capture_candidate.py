#!/usr/bin/env python3
"""Pencil2 saved-setting route candidate built from the pinned Pencil3 Drawing probe.

--preflight performs AST/hash checks only. It does not import Unicorn or any native
adapter. The explicit --capture mode is for a reviewed root run only. It records
native Model/Drawing route state and only decodes RTV4 geometry when the observed
selected redraw is GLV14; it never claims GPU pixels.
"""
from __future__ import annotations

import argparse
import ast
import builtins
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys
import traceback

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
PROBE = ROOT / "tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/pencil3_selector_probe_native.py"
LIBS = ROOT / "scratch/apk-analysis-native/arm64-v8a"
OUTPUTS = HERE / "captures"
PINNED_TOOLBIN = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"
PINNED_TOOL_LIBS = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
PINNED_PYTHONPATH = ROOT / "scratch/apk-analysis-runtime/python"
HOST_LIBM = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")

PINS = {
    "pencil3_selector_probe_native.py": (PROBE, "0212de3d9c6b3b5832587601c32cea087f6fbf537fe34b101985970a3107b907"),
    "oblique_canvas_native.py": (ROOT / "tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/oblique_canvas_native.py", "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2"),
    "fountain_native.py": (ROOT / "tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/fountain_native.py", "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30"),
    "elf_plt.py": (ROOT / "tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/elf_plt.py", "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b"),
    "pencil_native.py": (ROOT / "tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/pencil_native.py", "b4d88fb86da22c732e63466e6f1fc7c829aca9d0b2623abbcb3d0fc6f981795c"),
    "legacy-pencils-source.json": (HERE.parent / "legacy-pencils-source.json", "58dafbda19adcc5d4a22670a6c7e25a545f924ecb533bb61bc13b8a4a6c2f779"),
    "pencil2-v14-native-capture-plan.md": (HERE.parent / "pencil2-v14-native-capture-plan.md", "0e3b9e2f70f7f6a234587250354ae98b4a0cde4e405c3f50be68f290f5c357bf"),
    "libSPenModel.so": (LIBS / "libSPenModel.so", "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"),
    "libSPenPenCommon.so": (LIBS / "libSPenPenCommon.so", "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d"),
    "libSPenBase.so": (LIBS / "libSPenBase.so", "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"),
    "libSPenPencil2.so": (LIBS / "libSPenPencil2.so", "3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f"),
    "libSPenFountainPen.so": (LIBS / "libSPenFountainPen.so", "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec"),
    "libSPenDrawing.so": (LIBS / "libSPenDrawing.so", "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd"),
    "libSPenGraphics.so": (LIBS / "libSPenGraphics.so", "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621"),
    "libSPenComposer.so": (LIBS / "libSPenComposer.so", "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f"),
    "libc++_shared.so": (LIBS / "libc++_shared.so", "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"),
    "Unicorn __init__.py": (ROOT / "scratch/apk-analysis-runtime/python/unicorn/__init__.py", "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b"),
    "Unicorn arm64_const.py": (ROOT / "scratch/apk-analysis-runtime/python/unicorn/arm64_const.py", "92f91a2b0a36cd34deee16db4f8da4ac0c9ae35c3ea1089296e5ef9d170a5be9"),
    "Unicorn package source aggregate": (ROOT / "scratch/apk-analysis-runtime/python/unicorn", "TREE:c80b0d970ae57c0ba08ec143ff3264362511bd4e111690f9c81c54edfacd924c"),
    "libunicorn.so.2": (ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib/libunicorn.so.2", "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c"),
    "aarch64-linux-gnu-objdump": (PINNED_TOOLBIN / "aarch64-linux-gnu-objdump", "b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb"),
    "aarch64-linux-gnu-nm": (PINNED_TOOLBIN / "aarch64-linux-gnu-nm", "507950b76ed983334636dee2c9eba697d159e47918fdd0068a21c1b87f8f7116"),
    "aarch64-linux-gnu-c++filt": (PINNED_TOOLBIN / "aarch64-linux-gnu-c++filt", "6cd6e4dc10f82a713b312f523ec4e68b9ef5648ae670975f56da201243cd8656"),
    "libbfd-2.44-arm64.so": (PINNED_TOOL_LIBS / "libbfd-2.44-arm64.so", "10d63b821532b8cd6237b7d0b61ebe58ac3f58021a31329841492831f7567b74"),
    "libopcodes-2.44-arm64.so": (PINNED_TOOL_LIBS / "libopcodes-2.44-arm64.so", "13f97afd100836b1bac1502d5b8fe2e00f2e6f5843a81d711d628271c36531a7"),
    "libctf-arm64.so.0": (PINNED_TOOL_LIBS / "libctf-arm64.so.0", "e7ad4465110859c3817f911f50f600b46fb7a6e797f096ca18d9b2a266341ca6"),
    "libctf-nobfd-arm64.so.0": (PINNED_TOOL_LIBS / "libctf-nobfd-arm64.so.0", "f9235ceea727318bbb72471480071287e838516a018dc2e1ba60f87937d453c9"),
    "host nm": (Path("/usr/bin/nm"), "ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1"),
    "host c++filt": (Path("/usr/bin/c++filt"), "7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2"),
    "python3 interpreter": (Path("/usr/bin/python3"), "889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9"),
    "host libm.so.6": (HOST_LIBM, "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"),
    "V10 candidate source": (HERE.parent / "pencil2-v14-geometry-v10" / "capture_candidate.py", "40c99bcca08c606634430649f6a6492d76824afe1623155834a7090df67d6796"),
    "V10 baseline capture": (HERE.parent / "pencil2-v14-geometry-v10" / "captures" / "channel_baseline" / "capture.json", "469b54b231de1b6a8672c36130fda2e2617da53cfe90b0e188442610e2cd5eea"),
}

# Fresh V12 native controls: preserve the V10 baseline and vary exactly one
# saved input per case. Values are controlled Model inputs, not serialized units.
_CONTROL_BASELINE = {
    "points": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 10.0]],
    "size": 2.0,
    "curve_enabled": True,
    "tool_type": 2,
    "stroke_type": 0,
    "pressure": [0.5, 0.5, 0.5, 0.5],
    "timestamps": [10, 20, 30, 40],
    "tilt": [0.0, 0.0, 0.0, 0.0],
    "orientation": [0.0, 0.0, 0.0, 0.0],
    "color_argb": 0xff000000,
    "advanced_setting_arg": "14;",
    "advanced_setting_state": "explicit literal 14; set on ObjectStroke",
}
_GEOMETRY_CONTROL_DEFINITIONS = {
    "channel_baseline": ("v10_exact_channel_baseline", {}),
    "tilt_all_079": ("v10_baseline_tilt_all_079", {"tilt": [0.79, 0.79, 0.79, 0.79]}),
    "orientation_all_075": ("v10_baseline_orientation_all_075", {"orientation": [0.75, 0.75, 0.75, 0.75]}),
    "color_80336699": ("v10_baseline_color_80336699", {"color_argb": 0x80336699}),
    "size_8": ("v10_baseline_size_8", {"size": 8.0}),
}
CASES = {
    name: {**_CONTROL_BASELINE, **overrides, "case_id": case_id}
    for name, (case_id, overrides) in _GEOMETRY_CONTROL_DEFINITIONS.items()
}
GEOMETRY_AXIS_CASES = {
    "tilt_all_079": "tilt",
    "orientation_all_075": "orientation",
    "color_80336699": "color_argb",
    "size_8": "size",
}

REQUIRED_SOURCE_MARKERS = (
    "class PencilCanvasProbe(oc.CanvasProbe):",
    "SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)",
    "SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)",
    "SPen::Pencil3StrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, float, float)",
    "# Preserve register arguments and the packed RTV1 lane conversion as separate receipts.",
    "if not draw_stream or len(draw_stream)%20 or draw_stream!=saved_stream:",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def python_source_tree_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    for file_path in sorted(path.rglob("*.py")):
        if not file_path.is_file():
            continue
        relative = file_path.relative_to(ROOT).as_posix().encode()
        file_digest = sha256(file_path).encode()
        digest.update(relative + b"\0" + file_digest + b"\n")
    return digest.hexdigest()


def check_pins() -> dict[str, str]:
    actual: dict[str, str] = {}
    for name, (path, expected) in PINS.items():
        if expected.startswith("TREE:"):
            if not path.is_dir():
                raise SystemExit(f"missing pinned source tree {name}: {path}")
            digest = python_source_tree_sha256(path)
            wanted = expected.removeprefix("TREE:")
        else:
            if not path.is_file():
                raise SystemExit(f"missing pinned input {name}: {path}")
            digest = sha256(path)
            wanted = expected
        actual[name] = digest
        if digest != wanted:
            raise SystemExit(f"pin mismatch {name}: expected {wanted}, got {digest}")
    return actual


def preflight() -> None:
    hashes = check_pins()
    source = PROBE.read_text()
    tree = ast.parse(source, filename=str(PROBE))
    assert_geometry_control_matrix()
    assert_v10_baseline_join()
    missing = [marker for marker in REQUIRED_SOURCE_MARKERS if marker not in source]
    if missing:
        raise SystemExit("pinned probe no longer has required adaptation anchors: " + repr(missing))
    adapted_cases = {}
    for case_name, case in CASES.items():
        adapted = adapt_source(case)
        adapted_tree = ast.parse(adapted, filename=str(PROBE) + f"#Pencil2-{case_name}-adapted")
        assert_sincosf_adapter_contract(adapted_tree)
        assert_pencil2_rtv4_tail_contract(adapted)
        adapted_cases[case_name] = {
            "adapted_source_sha256": hashlib.sha256(adapted.encode()).hexdigest(),
            "top_level_statement_count": len(adapted_tree.body),
        }
    top_level = [node.name for node in tree.body if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
    report = {
        "schema": "pencil2-saved-setting-route-preflight-v1",
        "mode": "AST/hash-only; no native imports or execution",
        "probe": str(PROBE.relative_to(ROOT)),
        "probe_sha256": hashes["pencil3_selector_probe_native.py"],
        "pins": hashes,
        "required_adaptation_anchors": len(REQUIRED_SOURCE_MARKERS),
        "adapted_cases": adapted_cases,
        "top_level_functions_and_classes": top_level,
        "cases": list(CASES),
        "one_factor_controls": GEOMETRY_AXIS_CASES,
        "control_inputs": CASES,
        "v10_baseline_join": {
            "candidate_sha256": hashes["V10 candidate source"],
            "capture_sha256": hashes["V10 baseline capture"],
            "matched_native_pressure_alpha_returns": 5,
            "observer_contract": ["first-level 20 bytes", "second-level 20 bytes", "input S0", "return S0", "pending return count zero"],
        },
        "capture_boundary": "not run; --capture is a separate explicit mode",
    }
    print(json.dumps(report, indent=2, sort_keys=True))


def assert_geometry_control_matrix() -> None:
    baseline = CASES["channel_baseline"]
    if baseline["advanced_setting_arg"] != "14;" or baseline["advanced_setting_state"] != "explicit literal 14; set on ObjectStroke":
        raise SystemExit("every control must select Pencil2 GLV14 explicitly through Model input")
    expected = {
        "tilt_all_079": [0.79, 0.79, 0.79, 0.79],
        "orientation_all_075": [0.75, 0.75, 0.75, 0.75],
        "color_80336699": 0x80336699,
        "size_8": 8.0,
    }
    for case_name, varied_channel in GEOMETRY_AXIS_CASES.items():
        case = CASES[case_name]
        changed = {key for key in baseline if key != "case_id" and baseline[key] != case[key]}
        if changed != {varied_channel}:
            raise SystemExit(f"{case_name} must differ from the V10 baseline only in {varied_channel}, got {sorted(changed)}")
        if case[varied_channel] != expected[case_name]:
            raise SystemExit(f"{case_name} does not use the requested one-factor value")
        if case["advanced_setting_arg"] != "14;" or case["advanced_setting_state"] != baseline["advanced_setting_state"]:
            raise SystemExit(f"{case_name} does not preserve the explicit14 route")
    if any(value != 0.79 for value in CASES["tilt_all_079"]["tilt"][:1]):
        raise SystemExit("tilt control must include a nonzero first sample")
    if any(value != 0.75 for value in CASES["orientation_all_075"]["orientation"][:1]):
        raise SystemExit("orientation control must include a nonzero first sample")


def assert_v10_baseline_join() -> None:
    path, expected_sha = PINS["V10 baseline capture"]
    if sha256(path) != expected_sha:
        raise SystemExit("pinned V10 baseline capture hash changed")
    capture = json.loads(path.read_text())
    baseline = CASES["channel_baseline"]
    native = capture["input"]
    same = (
        native["object_stroke_size_f32_bits"] == "00000040"
        and native["object_stroke_color_argb"] == "0xff000000"
        and native["tool_type"] == baseline["tool_type"]
        and native["stroke_type"] == baseline["stroke_type"]
        and native["points"] == baseline["points"]
        and native["pressure"] == baseline["pressure"]
        and native["tilt"] == baseline["tilt"]
        and native["orientation"] == baseline["orientation"]
        and native["timestamps"] == baseline["timestamps"]
        and native["advanced_setting_state"] == baseline["advanced_setting_state"]
        and capture["route"]["selected_drawable_version"] == "v14"
        and capture["route"]["selected_drawable_class"] == "SPen::Pencil2StrokeDrawableGLV14"
        and capture["route"]["selected_rt_class"] == "Pencil2StrokeDrawableRTV4"
    )
    if not same:
        raise SystemExit("V12 baseline no longer joins the pinned V10 input/route")
    sources = [row["return"]["x0"] for row in capture["native_getters"]["motion_event_getter_returns"] if row["signature"] == "SPen::MotionEvent::GetSource() const"]
    alpha_rows = [row for row in capture["pressure_alpha_observations"] if row["phase"] == "draw_object_stroke"]
    if not sources or any(value != "0x0" for value in sources):
        raise SystemExit("V10 baseline MotionEvent source is not pinned to zero")
    if capture["pressure_alpha_pending_returns"] != 0 or len(alpha_rows) != 5:
        raise SystemExit("V10 baseline does not retain five matched pressure-alpha returns")
    if any(row["first_data_20_bytes"] != "186a031000000000010000000000000000000000" for row in alpha_rows):
        raise SystemExit("V10 baseline first-level getPressureAlpha state changed")
    if any(row["second_data_20_bytes"] != "00000040000000ff0000000000000000d0bf0e02" for row in alpha_rows):
        raise SystemExit("V10 baseline second-level getPressureAlpha state changed")
    if any(row["return_s0_bits"] != "0x3f0c4230" for row in alpha_rows):
        raise SystemExit("V10 baseline native pressure-alpha return changed")


def _replace_once(source: str, before: str, after: str, label: str) -> str:
    count = source.count(before)
    if count != 1:
        raise RuntimeError(f"fail-closed adaptation {label}: expected one anchor, found {count}")
    return source.replace(before, after, 1)


def assert_sincosf_adapter_contract(tree: ast.AST) -> None:
    """Pure AST contract checks for the host C float32 import bridge."""
    functions = [node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef)]
    callback = next((node for node in functions if node.name == "_pencil2_sincosf_callback"), None)
    if callback is None:
        raise SystemExit("adapted source lacks the typed sincosf callback")
    assignments = [
        node for node in ast.walk(tree)
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Attribute)
            and target.attr == "argtypes"
            and isinstance(target.value, ast.Name)
            and target.value.id == "_pencil2_sincosf"
            for target in node.targets
        )
    ]
    argtypes = next((node.value for node in assignments if isinstance(node.value, ast.List)), None)
    if not isinstance(argtypes, ast.List) or len(argtypes.elts) != 3:
        raise SystemExit("sincosf must declare three C arguments")
    def dumped(node: ast.AST) -> str:
        return ast.dump(node, include_attributes=False)
    expected = [
        "Attribute(value=Name(id='ctypes', ctx=Load()), attr='c_float', ctx=Load())",
        "Call(func=Attribute(value=Name(id='ctypes', ctx=Load()), attr='POINTER', ctx=Load()), args=[Attribute(value=Name(id='ctypes', ctx=Load()), attr='c_float', ctx=Load())])",
        "Call(func=Attribute(value=Name(id='ctypes', ctx=Load()), attr='POINTER', ctx=Load()), args=[Attribute(value=Name(id='ctypes', ctx=Load()), attr='c_float', ctx=Load())])",
    ]
    if [dumped(node) for node in argtypes.elts] != expected:
        raise SystemExit("sincosf must use float32 input and two float32 output pointers")
    restype_assignments = [
        node for node in ast.walk(tree)
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Attribute)
            and target.attr == "restype"
            and isinstance(target.value, ast.Name)
            and target.value.id == "_pencil2_sincosf"
            for target in node.targets
        )
    ]
    if len(restype_assignments) != 1 or not isinstance(restype_assignments[0].value, ast.Constant) or restype_assignments[0].value.value is not None:
        raise SystemExit("sincosf must be declared void-returning")
    calls = [node for node in ast.walk(callback) if isinstance(node, ast.Call)]
    bridge_calls = [node for node in calls if isinstance(node.func, ast.Name) and node.func.id == "_pencil2_sincosf"]
    if len(bridge_calls) != 1 or len(bridge_calls[0].args) != 3:
        raise SystemExit("sincosf callback must make one typed three-argument C call")
    guest_writes = [node for node in calls if isinstance(node.func, ast.Attribute) and node.func.attr == "mem_write"]
    if len(guest_writes) != 2 or any(len(node.args) != 2 for node in guest_writes):
        raise SystemExit("sincosf callback must write both float32 outputs to guest pointers")
    target_registers = {
        node.args[0].args[0].value
        for node in guest_writes
        if isinstance(node.args[0], ast.Call)
        and isinstance(node.args[0].func, ast.Attribute)
        and node.args[0].func.attr == "x"
        and node.args[0].args
        and isinstance(node.args[0].args[0], ast.Constant)
    }
    if target_registers != {0, 1}:
        raise SystemExit(f"sincosf callback output pointers must come from guest x0/x1, got {target_registers}")


def assert_pencil2_rtv4_tail_contract(source: str) -> None:
    """Reject inherited Pencil3 five-lane tail logic in every adapted case."""
    required = (
        "len(stream)%56",
        "len(events)!=len(stream)//56",
        "if draw_stream!=saved_stream:",
        "n-before!=56",
        "struct.unpack('<14f',delta)",
        "record_stride_bytes':56",
        "'same_as_drawing':saved_stream==draw_stream",
        "'case_id':input_case['case_id']",
        "'factory_return_snapshots':factory_return_snapshots",
        "'selected_drawable_version':selected_drawable_version",
        "'style_calls':style_calls",
        "streams={'selected_version':selected_drawable_version,'rtv4_capture_skipped':not is_v14}",
        "Pencil2StrokeDrawableRTV4::AddPoint(",
        "SPen::Pencil2StrokeDrawableGLV14::getPressureAlpha(float)",
        "pressure_alpha_observations.append(alpha_row)",
        "first_data_20_bytes",
        "second_data_20_bytes",
        "return_s0_bits",
        "unmatched getPressureAlpha return observations",
    )
    missing = [marker for marker in required if marker not in source]
    if missing:
        raise SystemExit("adapted Pencil2 RTV4 tail lacks source-pinned 56-byte checks: " + repr(missing))
    retired = (
        "len(draw_stream)%20",
        "len(draw_stream)//20",
        "48*5",
        "float_args",
        "rtv1_packed",
        "Pencil2StrokeDrawableRTV1",
        "Pencil2StrokeDrawableGLV21",
        "ShaderV3",
        "case_id='straight_curve_tool2_'",
        "selected_drawable_version=='v1' and any(len(profile['seed_values'])!=1",
        "selected_drawable_version=='v3' and any(profile['seed_values'] or profile['rand_values']",
        "selected_drawable_version=='v1'",
        "selected_drawable_version=='v3'",
        "incomplete modeled RNG trace for V1",
        "unexpected RNG import calls for V3",
    )
    present = [marker for marker in retired if marker in source]
    if present:
        raise SystemExit("adapted Pencil2 source retains retired Pencil3/RTV1 tail assumptions: " + repr(present))

    tree = ast.parse(source)
    stream_reads = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Assign):
            continue
        if not any(isinstance(target, ast.Name) and target.id in ("draw_stream", "saved_stream") for target in node.targets):
            continue
        value = node.value
        if isinstance(value, ast.Call) and isinstance(value.func, ast.Attribute) and value.func.attr == "fromhex":
            stream_reads.append(node)
    v14_blocks = [
        node for node in ast.walk(tree)
        if isinstance(node, ast.If) and isinstance(node.test, ast.Name) and node.test.id == "is_v14"
    ]
    stream_gates = [block for block in v14_blocks if all(candidate in ast.walk(block) for candidate in stream_reads)]
    if len(stream_reads) != 2 or len(stream_gates) != 1:
        raise SystemExit(f"expected exactly two stream reads under one selected-v14 gate, got reads={len(stream_reads)} gates={len(stream_gates)}")
    gated_reads = [candidate for candidate in stream_reads if candidate in ast.walk(stream_gates[0])]
    if len(gated_reads) != 2:
        raise SystemExit("Drawing/replay RTV4 stream extraction must occur only inside the selected GLV14 branch")


P2_BUFFER_HOOK = r''' if a==p.syms.get('SPen::Pencil2StrokeDrawableGLV14::getPressureAlpha(float)'):
  alpha_this=u.reg_read(R.UC_ARM64_REG_X0); first_data=struct.unpack('<Q',u.mem_read(alpha_this+72,8))[0]
  second_data=struct.unpack('<Q',u.mem_read(first_data,8))[0]
  alpha_lr=u.reg_read(R.UC_ARM64_REG_LR)
  alpha_row={'phase':phase,'entry_symbol':'SPen::Pencil2StrokeDrawableGLV14::getPressureAlpha(float)','entry_rva':'0x744c4','entry_pc':hex(a),'entry_word':bytes(u.mem_read(a,4)).hex(),'this':hex(alpha_this),'input_s0_bits':hex(u.reg_read(R.UC_ARM64_REG_S0)),'lr':hex(alpha_lr),'first_data_pointer':hex(first_data),'first_data_20_bytes':bytes(u.mem_read(first_data,20)).hex(),'second_data_pointer':hex(second_data),'second_data_20_bytes':bytes(u.mem_read(second_data,20)).hex(),'size_level_i32':bytes(u.mem_read(alpha_this+820,4)).hex()}
  pressure_alpha_observations.append(alpha_row)
  pressure_alpha_pending.append({'lr':alpha_lr,'row':alpha_row})
 for pending_alpha in list(pressure_alpha_pending):
  if pending_alpha['lr']==a:
   pending_alpha['row']['return_s0_bits']=hex(u.reg_read(R.UC_ARM64_REG_S0))
   pressure_alpha_pending.remove(pending_alpha)
 if a==p.syms.get('SPen::Pencil2StrokeDrawableRTV4::Clear()'):
  event_order.append({'event':'RTV4::Clear','phase':phase,'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'pc':hex(a)})
 if a==p.syms.get('SPen::Pencil2StrokeDrawableRTV4::Update(bool)'):
  event_order.append({'event':'RTV4::Update','phase':phase,'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'pc':hex(a),'arg_bool':u.reg_read(R.UC_ARM64_REG_W1)})
 if a==p.syms.get('SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'):
  event_order.append({'event':'RTV4::Draw','phase':phase,'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'pc':hex(a)})
 if a==p.syms.get('SPen::Pencil2StrokeDrawableRTV4::AddPoint(SPen::Vector2<float>*, float, float, float, int, SPen::Vector3<float>)'):
  this=u.reg_read(R.UC_ARM64_REG_X0); point=u.reg_read(R.UC_ARM64_REG_X1)
  vector_obj=struct.unpack('<Q',u.mem_read(this+80,8))[0]
  bounds=struct.unpack('<3Q',u.mem_read(vector_obj,24)) if vector_obj else (0,0,0)
  begin,end,capacity=bounds; n=end-begin
  if not (begin<=end<=capacity) or n>1_000_000 or n%4: raise RuntimeError(f'invalid Pencil2 RTV4 vector descriptor {bounds!r}')
  regs={f's{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(7)}
  record={'phase':phase,'rt':hex(this),'vector_object':hex(vector_obj),'before_count_bytes':n,'before_bytes':bytes(u.mem_read(begin,n)).hex() if n else '', 'vector2_pointer':hex(point),'vector2_bytes':bytes(u.mem_read(point,8)).hex() if point else None,'int_arg_w2':u.reg_read(R.UC_ARM64_REG_W2),'float_register_bits':regs,'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))}
  event_order.append({'event':'RTV4::AddPoint','phase':phase,'this':hex(this),'pc':hex(a),'vector2':record['vector2_bytes'],'w2':record['int_arg_w2'],'s0_s6':regs})
  pending_pencil_buffer_returns[u.reg_read(R.UC_ARM64_REG_LR)]=(vector_obj,record)
 if a in pending_pencil_buffer_returns:
  vector_obj,record=pending_pencil_buffer_returns.pop(a)
  begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); n=end-begin; before=record['before_count_bytes']
  if not (begin<=end<=capacity) or n>1_000_000 or n%4 or n<before: raise RuntimeError(f'invalid Pencil2 RTV4 AddPoint return descriptor {begin:#x}/{end:#x}/{capacity:#x}')
  if n-before!=56: raise RuntimeError(f'RTV4 AddPoint produced {n-before} bytes; expected statically established 14-f32 record (56 bytes)')
  delta=bytes(u.mem_read(begin+before,56))
  values=struct.unpack('<14f',delta)
  if any(not math.isfinite(value) for value in values): raise RuntimeError(f'non-finite RTV4 AddPoint payload {values!r}')
  record.update({'after_count_bytes':n,'after_bytes':bytes(u.mem_read(begin,n)).hex() if n else '', 'delta_bytes':delta.hex(),'delta_f32':list(values)})
  pencil_buffer_events.append(record)
'''

P2_SINCOSF_ADAPTER = r'''import ctypes
_pencil2_libm=ctypes.CDLL('/usr/lib/x86_64-linux-gnu/libm.so.6')
_pencil2_sincosf=_pencil2_libm.sincosf
_pencil2_sincosf.argtypes=[ctypes.c_float,ctypes.POINTER(ctypes.c_float),ctypes.POINTER(ctypes.c_float)]
_pencil2_sincosf.restype=None
p._sincosf_trace=[]
def _pencil2_sincosf_callback():
 angle=p.f(0)
 sin_value=ctypes.c_float(); cos_value=ctypes.c_float()
 _pencil2_sincosf(ctypes.c_float(angle),ctypes.byref(sin_value),ctypes.byref(cos_value))
 p.u.mem_write(p.x(0),struct.pack('<f',sin_value.value))
 p.u.mem_write(p.x(1),struct.pack('<f',cos_value.value))
 p._sincosf_trace.append({'phase':phase,'input_f32_bits':struct.pack('<f',angle).hex(),'sin_f32_bits':struct.pack('<f',sin_value.value).hex(),'cos_f32_bits':struct.pack('<f',cos_value.value).hex()})
p.callbacks['sincosf']=_pencil2_sincosf_callback
'''


def adapt_source(case: dict[str, object]) -> str:
    source = PROBE.read_text()
    for marker in REQUIRED_SOURCE_MARKERS:
        if marker not in source:
            raise RuntimeError(f"missing source marker: {marker}")

    # The source pin map belongs to the probe's original helper set. Repoint
    # only its explicit Pencil3 content pin and routed library/factory names.
    source = source.replace("Pencil3", "Pencil2")
    source = _replace_once(
        source,
        "import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json",
        "import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json,math",
        "finite output validation dependency",
    )
    source = _replace_once(
        source,
        "p.callbacks['srand']=_seed_pencil_rng; p.callbacks['rand']=_next_pencil_rng\n",
        "p.callbacks['srand']=_seed_pencil_rng; p.callbacks['rand']=_next_pencil_rng\n" + P2_SINCOSF_ADAPTER,
        "install typed sincosf import bridge",
    )
    source = _replace_once(
        source,
        "FIXTURE=ROOT/'conformance/pencil3-saved-caller-native.json'",
        "FIXTURE=pathlib.Path(PENCIL2_OUTPUT_DIR)/'disabled-legacy-probe-fixture.json'",
        "isolate adapted probe fixture path",
    )
    source = _replace_once(
        source,
        "if args.check_fixture or args.write_fixture:\n cases={}",
        "if args.check_fixture or args.write_fixture:\n raise SystemExit('fixture read/write modes are disabled in this candidate')\n cases={}",
        "forbid adapted probe fixture read/write modes",
    )
    source = _replace_once(
        source,
        "oc.native.HASHES['Pencil2']='41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359'",
        "oc.native.HASHES['Pencil2']='3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f'",
        "Pencil2 DSO hash",
    )
    source = _replace_once(
        source,
        "p.syms.get('SPen::Pencil2StrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, float, float)')",
        "p.syms.get('SPen::Pencil2StrokeDrawableRTV4::AddPoint(SPen::Vector2<float>*, float, float, float, int, SPen::Vector3<float>)')",
        "RTV4 AddPoint hook",
    )
    source = _replace_once(
        source,
        "'SPen::Pencil2StrokeDrawableRTV1::ResetPointOffset()'",
        "'SPen::Pencil2StrokeDrawableRTV4::Clear()'",
        "RTV4 reset hook",
    )
    source = _replace_once(
        source,
        "addpoint_method=_name.startswith('SPen::Pencil2StrokeDrawableRT') and any(token in _name for token in ('::AddPoint(','::ResetPointOffset(','::GetPointOffset('))",
        "addpoint_method=_name.startswith('SPen::Pencil2StrokeDrawableRT') and any(token in _name for token in ('::AddPoint(','::Clear()','::Update(bool)','::Draw('))",
        "RTV4 lifecycle tracing",
    )
    source = _replace_once(source, "for v in (1,2,3,4) if any(f'Pencil2StrokeDrawableGLV{v}::RedrawPen'", "for v in (1,2,3,4,5,7,8,9,10,11,12,13,14) if any(f'Pencil2StrokeDrawableGLV{v}::RedrawPen'", "Pencil2 selector version reporting")
    source = _replace_once(source, "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}", "input_case=" + repr(case), "controlled Model input")
    source = _replace_once(
        source,
        "case_id='straight_curve_tool2_'+args.advanced_setting\ninput_case=",
        "case_id=None\ninput_case=",
        "remove inherited generic Pencil3 case label",
    )
    source = _replace_once(source, "choices=('null','empty','0;','1;','2;','3;','4;','basepen')", "choices=('null','empty','0;','1;','2;','3;','4;','14;','basepen')", "explicit 14 setting control")

    # The actual ObjectStroke color is explicit black; retain no advanced
    # setting setter so Pencil2's native constructor-selected 14; remains.
    source = _replace_once(
        source,
        " p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);",
        " p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,input_case['color_argb']]); p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);",
        "black control color",
    )
    source = _replace_once(source, "'SPen::ObjectStroke::GetOrientation() const','SPen::ObjectStroke::GetPenSize() const'", "'SPen::ObjectStroke::GetOrientation() const','SPen::ObjectStroke::GetSizeLevel() const','SPen::ObjectStroke::GetPenSize() const'", "size-level getter pin")
    source = _replace_once(source, "'point_count':count,'curve_enabled':curve", "'point_count':count,'size_level':get_i('SPen::ObjectStroke::GetSizeLevel() const'),'curve_enabled':curve", "size-level snapshot")
    source = _replace_once(
        source,
        "or _name in ('SPen::Pen::SetColor(int)','SPen::PenDrawableRT::SetPenData(float, int)')",
        "or _name in ('SPen::Pen::SetColor(int)','SPen::Pen::SetSize(float)','SPen::Pen::SetSizeLevel(int)','SPen::Pen::SetAdvancedSetting(SPen::String const*)','SPen::PenDrawableRT::SetPenData(float, int)')",
        "style method entry tracing",
    )
    hook_start = source.index(" if a==p.syms.get('SPen::Pencil2StrokeDrawableRTV4::AddPoint(")
    hook_end = source.index("p.u.hook_add(UC_HOOK_CODE,log_vcall)", hook_start)
    source = source[:hook_start] + P2_BUFFER_HOOK + source[hook_end:]

    # Add style setter traces to the existing native call entry collector.
    source = _replace_once(source, "paint_color_calls=[]\npending_pencil_buffer_returns={}", "paint_color_calls=[]\nstyle_calls=[]\npending_pencil_buffer_returns={}", "style receipt state")
    source = _replace_once(
        source,
        "  native_draw_entries.append(record)\n  for signature in method_trace_names_by_pc[a]:",
        "  for signature in method_trace_names_by_pc[a]:\n   if signature in ('SPen::Pen::SetSize(float)','SPen::Pen::SetSizeLevel(int)','SPen::Pen::SetColor(int)','SPen::Pen::SetAdvancedSetting(SPen::String const*)','SPen::PenDrawableRT::SetPenData(float, int)'):\n    style_calls.append({'method':signature,'phase':phase,'entry_pc':hex(a),'x0':hex(u.reg_read(R.UC_ARM64_REG_X0)),'x1':hex(u.reg_read(R.UC_ARM64_REG_X1)),'s0':hex(u.reg_read(R.UC_ARM64_REG_S0)),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  native_draw_entries.append(record)\n  for signature in method_trace_names_by_pc[a]:",
        "style setter tracing",
    )
    source = _replace_once(
        source,
        "event_order=[]",
        "event_order=[]\npressure_alpha_observations=[]\npressure_alpha_pending=[]",
        "initialize getPressureAlpha observer list",
    )
    source = _replace_once(
        source,
        "selected_pen=factory_created_pens[-1]; p.call('SPen::Pen::getSettingData()',[selected_pen]); actual_setting_data=p.x(0); setting_size_bits=bytes(p.u.mem_read(actual_setting_data,4)).hex(); setting_resolution=list(struct.unpack('<ii',p.u.mem_read(actual_setting_data+32,8)));\n if setting_resolution != [args.width,args.height] or setting_size_bits != source_size_bits: raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')",
        "selected_pen=factory_created_pens[-1]; p.call('SPen::Pen::getSettingData()',[selected_pen]); actual_setting_data=p.x(0); setting_size_bits=bytes(p.u.mem_read(actual_setting_data,4)).hex(); setting_resolution=list(struct.unpack('<ii',p.u.mem_read(actual_setting_data+32,8)));\n native_object_size=struct.unpack('<f',bytes.fromhex(source_size_bits))[0]; expected_effective_size_bits=struct.pack('<f',min(max(native_object_size,0.4),800.0)).hex(); pen_size_contract={'object_stroke_getpensize_f32_bits':source_size_bits,'pencil2_constructor_getsize_f32_bits':pencil_ctor_size_bits,'pen_setting_data_offset0_f32_bits':setting_size_bits,'expected_effective_pen_size_f32_bits_from_pencommon_clamp':expected_effective_size_bits,'expected_effective_size_matches':setting_size_bits==expected_effective_size_bits,'native_screen_resolution_i32_offsets_32_36':setting_resolution,'profile_matches':setting_resolution==[args.width,args.height],'native_pen_size_setter_entries':style_calls}",
        "record raw ObjectStroke versus effective native Pen size without aborting capture",
    )
    source = _replace_once(
        source,
        "case_id='straight_curve_tool2_'+args.advanced_setting\n draw_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes'])",
        "case_id=input_case['case_id']\n draw_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes'])",
        "Pencil2 fixture case identity",
    )
    source = _replace_once(
        source,
        "if not draw_stream or len(draw_stream)%20 or draw_stream!=saved_stream: raise RuntimeError(f'expected matching nonempty 20-byte AddPoint records, got {len(draw_stream)} and {len(saved_stream)} bytes')",
        "if not draw_stream or len(draw_stream)%56 or draw_stream!=saved_stream: raise RuntimeError(f'expected matching nonempty 56-byte RTV4 records, got {len(draw_stream)} and {len(saved_stream)} bytes')",
        "Pencil2 RTV4 native stream stride",
    )
    source = _replace_once(
        source,
        "  if len(events)!=len(draw_stream)//20 or any(int(row['rt'],16)!=selected_rt for row in events):",
        "  if len(events)!=len(draw_stream)//56 or any(int(row['rt'],16)!=selected_rt for row in events):",
        "Pencil2 RTV4 AddPoint event count",
    )
    stream_start = source.index("case_id=input_case['case_id']\n draw_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes'])")
    stream_end = source.index(" if not event_order or event_order[0]['event']!='Pen::SetScreenResolution':", stream_start)
    source = source[:stream_start] + "case_id=input_case['case_id']\n draw_stream=b''\n saved_stream=b''\n phase_counts={}\n" + source[stream_end:]
    source = _replace_once(
        source,
        " if selected_drawable_version=='v1' and any(len(profile['seed_values'])!=1 or not profile['rand_values'] for profile in rng_by_phase.values()): raise RuntimeError(f'incomplete modeled RNG trace for V1: {rng_by_phase}')",
        " # Pencil2 RNG callback data remains a raw observation without version-specific assertions.",
        "remove inherited Pencil3 V1 RNG assertion",
    )
    source = _replace_once(
        source,
        " if selected_drawable_version=='v3' and any(profile['seed_values'] or profile['rand_values'] for profile in rng_by_phase.values()): raise RuntimeError(f'unexpected RNG import calls for V3: {rng_by_phase}')",
        " # Pencil2 RNG callback data remains a raw observation without version-specific assertions.",
        "remove inherited Pencil3 V3 RNG assertion",
    )
    source = source.replace("elif lib=='Pencil2' and where in (0x6c278,0x6c280):print('unresolved',hex(where),name,hex(val),hex(typ))\n", "")
    source = source.replace("if lib=='Pencil2' and where in (0x6c278,0x6c280):print('reloc',hex(where),name,hex(target),hex(base+where),p.u.mem_read(base+where,8).hex())\n", "")

    # Replace Pencil3-only lane decoding and result tail with strict Pencil2
    # route/version/RTV4 assertions and complete native buffer receipts.
    tail_start = source.index(" result={'schema':'pencil3-saved-caller-native-v1'")
    source = source[:tail_start] + " result={}\n" + r''' # Pencil2 v14 native geometry receipt; no GPU/pixel claims.
 if draw_result != 1 or saved_redraw_result != 1:
  raise RuntimeError(f'native Pencil2 Drawing/replay failed: {draw_result}, {saved_redraw_result}')
 is_v14=selected_drawable_version=='v14'
 selected_drawable_class=f'SPen::Pencil2StrokeDrawableGL{selected_drawable_version.upper()}' if selected_drawable_version!='unknown' else None
 rt_vtable=None
 if is_v14:
  rt_vtable=struct.unpack('<Q',p.u.mem_read(selected_rt,8))[0]
  expected_rt_vtable=rawsyms.get('_ZTVN4SPen25Pencil2StrokeDrawableRTV4E',0)
  # ELF vtable symbols point at the two-entry header; object vptrs point at
  # the first function slot, 16 bytes after the symbol.
  if not expected_rt_vtable or rt_vtable != expected_rt_vtable+16:
   raise RuntimeError(f'expected native RTV4 vptr {expected_rt_vtable+16:#x}, got {rt_vtable:#x}')
 elif pencil_buffer_events:
  raise RuntimeError(f'RTV4 hook fired though Drawing selected {selected_drawable_version}: {pencil_buffer_events}')
 advanced_calls=[row for row in style_calls if row['method']=='SPen::Pen::SetAdvancedSetting(SPen::String const*)']
 for row in advanced_calls:
  lr=int(row['lr'],16)
  caller_address=max((address for address in p.syms.values() if address<=lr),default=0)
  row['caller_symbols']=[name for name,address in p.syms.items() if address==caller_address]
 draw_stream=b''
 saved_stream=b''
 streams={'selected_version':selected_drawable_version,'rtv4_capture_skipped':not is_v14}
 if is_v14:
  for phase_name in ('draw_object_stroke','saved_redraw'):
   events=[row for row in pencil_buffer_events if row['phase']==phase_name]
   stream=bytes.fromhex(next(row for row in reversed(events) if row['phase']==phase_name)['after_bytes']) if events else b''
   if not events or len(stream)%56 or len(events)!=len(stream)//56:
    raise RuntimeError(f'incomplete RTV4 buffer in {phase_name}: events={len(events)} bytes={len(stream)}')
   if any(int(row['rt'],16)!=selected_rt for row in events):
    raise RuntimeError(f'RT pointer changed during {phase_name}')
  draw_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes'])
  saved_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='saved_redraw')['after_bytes'])
  if draw_stream!=saved_stream:
   raise RuntimeError('Drawing-produced and explicit saved ObjectStroke replay RTV4 buffers differ')
  streams={'selected_version':'v14','rtv4_capture_skipped':False,'draw_object_stroke':{'addpoint_calls':len([row for row in pencil_buffer_events if row['phase']=='draw_object_stroke']),'bytes':len(draw_stream),'record_stride_bytes':56,'record_stride_f32':14,'sha256':hashlib.sha256(draw_stream).hexdigest(),'data_hex':draw_stream.hex()},'saved_redraw':{'addpoint_calls':len([row for row in pencil_buffer_events if row['phase']=='saved_redraw']),'bytes':len(saved_stream),'record_stride_bytes':56,'record_stride_f32':14,'sha256':hashlib.sha256(saved_stream).hexdigest(),'data_hex':saved_stream.hex(),'same_as_drawing':saved_stream==draw_stream}}
 if screen_resolution_calls != [{'width':args.width,'height':args.height}]:
  raise RuntimeError(f'unexpected screen profile calls: {screen_resolution_calls}')
 if any(pending_motion_getters.values()):
  raise RuntimeError(f'MotionEvent getter return trace incomplete: {pending_motion_getters}')
 if any(not math.isfinite(float(value)) for channel in ('pressure','tilt','orientation') for value in input_case[channel]):
  raise RuntimeError('non-finite controlled Model input')
 if pressure_alpha_pending: raise RuntimeError(f'unmatched getPressureAlpha return observations: {len(pressure_alpha_pending)}')
 result.update({
  'schema':'pencil2-saved-caller-setting-route-geometry-v1',
  'case_id':input_case['case_id'],
  'pen_size_contract':pen_size_contract,
  'input':{'kind':'synthetic native Model ObjectStroke setter input; no serialized Samsung document','pen_name':'com.samsung.android.sdk.pen.pen.preload.Pencil2','tool_type':input_case['tool_type'],'stroke_type':input_case['stroke_type'],'object_stroke_color_argb':hex(input_case['color_argb']),'object_stroke_size_f32_bits':source_size_bits,'points':input_case['points'],'pressure':input_case['pressure'],'timestamps':input_case['timestamps'],'tilt':input_case['tilt'],'orientation':input_case['orientation'],'size_level_native_getter':object_stroke_snapshots[-1]['size_level'],'advanced_setting_state':input_case['advanced_setting_state'],'advanced_setting_model_setter_argument':input_case['advanced_setting_arg'],'advanced_getter_pointer_nonnull':bool(advanced_getter),'advanced_getter_utf8':advanced_utf8,'advanced_getter_object_hex':advanced_getter_bytes},
  'route':{'factory':'createPenInst reached through native Drawing route','pencil2_constructor_version_claim':'native Pencil2 constructor sets version 14 before Drawing applies ObjectStroke settings; this is constructor evidence only','constructor_size_f32_bits':pencil_ctor_size_bits,'factory_return_snapshots':factory_return_snapshots,'drawing_advanced_setting_calls':advanced_calls,'selected_drawable_class':selected_drawable_class,'selected_drawable_version':selected_drawable_version,'selected_rt_class':'Pencil2StrokeDrawableRTV4' if is_v14 else None,'selected_rt_vptr':hex(rt_vtable) if rt_vtable is not None else None,'selected_rt':hex(selected_rt) if selected_rt else None,'saved_redraw_entry':selected_object_redraw_names[0] if selected_object_redraw_names else None,'profile':{'width':args.width,'height':args.height}},
  'streams':streams,
  'addpoint_events':pencil_buffer_events,
  'selector_receipt':{'pencil2_get_stroke_drawable_gl_va':'0x59d6c','selected_drawable_class':selected_drawable_class,'selected_drawable_version':selected_drawable_version,'selected_redraw_entry_va':hex(selected_object_redraw_target),'rtv4_addpoint_entry_va':hex(p.syms['SPen::Pencil2StrokeDrawableRTV4::AddPoint(SPen::Vector2<float>*, float, float, float, int, SPen::Vector3<float>)']) if is_v14 else None,'rtv4_elf_vtable_symbol_va':hex(expected_rt_vtable) if is_v14 else None,'rtv4_vtable_address_point_va':hex(rt_vtable) if rt_vtable is not None else None,'rt_vector_descriptor_offset_bytes':80 if is_v14 else None,'rtv4_update_va':'0x995f4' if is_v14 else None,'static_record_stride_f32':14 if is_v14 else None,'static_record_stride_bytes':56 if is_v14 else None},
  'native_source_basis':{'pencil2_dso_sha256':'3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f','constructor_va':'0x59880','get_stroke_drawable_gl_va':'0x59d6c','glv14_redraw_va':'0x73028','glv14_draw_point_va':'0x74634','rtv4_addpoint_va':'0x98a98','get_pressure_alpha_symbol':'SPen::Pencil2StrokeDrawableGLV14::getPressureAlpha(float)','get_pressure_alpha_rva':'0x744c4','observer_revision':'V10_second_dereference_and_return_capture','get_pressure_alpha_entry_word':'082440f9','get_pressure_alpha_data_indirection':'first_data=*(this+72); second_data=*(first_data+0); base/alphaByte read from second_data; flag from first_data+16','rtv4_clear_va':'0x995d8','rtv4_update_va':'0x995f4','rtv4_draw_va':'0x99660','rtv4_vector_descriptor_offset_bytes':80,'rtv4_elf_vtable_symbol_va':'0xbc3a8','rtv4_vtable_address_point_va':'0xbc3b8','rtv4_update_record_divisor_f32':14,'source_receipt':'tmp/vector-parity-plan/pen-family-audit/legacy-pencils-source.json','capture_plan':'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-native-capture-plan.md'},
  'style_calls':style_calls,
  'pressure_alpha_observations':pressure_alpha_observations,'pressure_alpha_pending_returns':len(pressure_alpha_pending),
  'lifecycle_events':[row for row in event_order if row['event'].startswith('RTV4::')],
  'native_getters':{'source_arrays':source_object_arrays,'motion_event_getter_returns':native_motion_getter_returns,'motion_event_constructors':[{'phase':row['phase'],'signatures':row['signatures'],'pc':row['pc'],'regs':row['regs']} for row in motion_ctors]},
  'rng_observations':{'by_phase':rng_by_phase,'calls':_rng_trace,'interpretation':'raw calls from the pinned parent probe callback model; no unrelated version-specific assertion is applied','parity_boundary':'modeled host callback outputs are not Samsung device RNG evidence'},
  'profile':{'width':args.width,'height':args.height,'evidence_supplied_by_root':PROFILE_EVIDENCE,'kind':'explicit controlled IContext profile; not inferred device dimensions'},
  'host_boundaries':['Each --capture invocation constructs a fresh native probe and Pencil2 Model/Drawing route; run unset, empty, and explicit14 as separate processes. The ObjectStroke getter and Drawing-supplied advanced-setting call are recorded, so constructor state is not conflated with the final selected drawable version.','Canvas/bitmap/context, task queue, loader boundary, allocator and non-target system interfaces are the explicit existing probe adapters. sincosf is bridged to the pinned host glibc libm.so.6 C float32 function with x0/x1 guest output pointers; trig values are host-libm evidence, not Android/Samsung libm parity. RTV4 bytes are parsed only when the observed selected redraw is GLV14; no native RTV4 GPU Draw or pixel claim is made.'],
  'host_math_imports':{'sincosf':{'symbol':'sincosf(float, float*, float*)','implementation':'ctypes call to host C libm sincosf; binary32 input and outputs','path':'/usr/lib/x86_64-linux-gnu/libm.so.6','sha256':'6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3','guest_abi':'S0 float input; X0 points to sine float32 output; X1 points to cosine float32 output','calls':p._sincosf_trace,'parity_boundary':'host libm only; no Android/Samsung libm parity claim'}},
  'pixel_evidence':None,
 })
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 print(encoded,end='',_always=True)
except Exception:
 raise
'''
    if "Pencil3" in source or "libSPenPencil3.so" in source or "RTV1" in source:
        leftovers = [line for line in source.splitlines() if "Pencil3" in line or "libSPenPencil3.so" in line or "RTV1" in line]
        raise RuntimeError("Pencil3/RTV1 code path remains after adaptation: " + repr(leftovers))
    compile(source, str(PROBE), "exec")
    return source


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--preflight", action="store_true", help="AST/hash checks only; no native imports")
    mode.add_argument("--capture", action="store_true", help="explicit reviewed root-only native geometry capture")
    parser.add_argument("--case", choices=tuple(CASES))
    parser.add_argument("--width", type=int)
    parser.add_argument("--height", type=int)
    parser.add_argument("--profile-evidence", help="source/capture identifying this explicit context profile")
    args = parser.parse_args()
    if args.preflight:
        preflight()
        return
    if not args.capture:
        raise SystemExit("capture mode required")
    if not args.case or not args.width or not args.height or not args.profile_evidence:
        raise SystemExit("--capture requires --case, positive --width/--height, and --profile-evidence")
    if not (0 < args.width < 65536 and 0 < args.height < 65536):
        raise SystemExit("profile dimensions must be positive u16 values")
    if not args.profile_evidence.strip():
        raise SystemExit("profile provenance must be non-empty")
    if os.environ.get("PYTHONPATH") != str(PINNED_PYTHONPATH):
        raise SystemExit(f"capture requires the pinned PYTHONPATH exactly: {PINNED_PYTHONPATH}")
    capture_dir = OUTPUTS / args.case
    destination = capture_dir / "capture.json"
    failure_path = capture_dir / "failure.json"
    if not HERE.is_dir() or HERE.is_symlink():
        raise SystemExit(f"candidate directory is not a real directory: {HERE}")
    if OUTPUTS.exists() and (not OUTPUTS.is_dir() or OUTPUTS.is_symlink()):
        raise SystemExit(f"capture root must be a real, non-symlink directory: {OUTPUTS}")
    if capture_dir.exists() or capture_dir.is_symlink():
        raise SystemExit(f"refusing to reuse non-fresh capture directory: {capture_dir}")
    pinned_inputs = check_pins()
    source = adapt_source(CASES[args.case])
    import contextlib
    import io
    OUTPUTS.mkdir(parents=True, exist_ok=True)
    if not OUTPUTS.is_dir() or OUTPUTS.is_symlink():
        raise SystemExit(f"capture root must be a real, non-symlink directory: {OUTPUTS}")
    capture_dir.mkdir(exist_ok=False)
    if not capture_dir.is_dir() or capture_dir.is_symlink():
        raise SystemExit(f"capture parent must be a fresh, real, non-symlink directory: {capture_dir}")
    namespace = {
        "__name__": "__main__",
        "__file__": str(PROBE),
        "PROFILE_EVIDENCE": args.profile_evidence,
        "PENCIL2_OUTPUT_DIR": capture_dir,
    }
    old_argv = sys.argv
    old_path = os.environ.get("PATH")
    old_ld_library_path = os.environ.get("LD_LIBRARY_PATH")
    os.environ["PATH"] = str(PINNED_TOOLBIN) + ":/usr/bin:/bin"
    os.environ["LD_LIBRARY_PATH"] = str(PINNED_TOOL_LIBS)
    sys.argv = [str(PROBE), "--print-capture", "--advanced-setting", CASES[args.case]["advanced_setting_arg"], "--width", str(args.width), "--height", str(args.height)]
    captured_stdout = io.StringIO()
    try:
        try:
            # Native/emulation imports happen only after explicit --capture.
            with contextlib.redirect_stdout(captured_stdout):
                exec(compile(source, str(PROBE), "exec"), namespace)
            receipt = json.loads(captured_stdout.getvalue())
            receipt["candidate_source"] = str(Path(__file__).resolve().relative_to(ROOT))
            receipt["adapted_probe_source_sha256"] = hashlib.sha256(source.encode()).hexdigest()
            receipt["pinned_inputs"] = pinned_inputs
            with destination.open("x") as output_file:
                output_file.write(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        except BaseException as error:
            failure = {
                "schema": "pencil2-native-capture-failure-v1",
                "case": args.case,
                "candidate_sha256": sha256(Path(__file__)),
                "adapted_probe_source_sha256": hashlib.sha256(source.encode()).hexdigest(),
                "pinned_inputs": pinned_inputs,
                "exception_type": type(error).__name__,
                "exception_repr": repr(error)[:4000],
                "stdout_tail": captured_stdout.getvalue()[-12000:],
                "traceback_tail": traceback.format_exc()[-12000:],
                "pixels_or_geometry_receipt_written": destination.exists(),
            }
            try:
                probe = namespace.get("p")
                registers = namespace.get("R")
                if probe is not None and registers is not None:
                    pc = probe.u.reg_read(registers.UC_ARM64_REG_PC)
                    lr = probe.u.reg_read(registers.UC_ARM64_REG_LR)
                    failure["pc"] = hex(pc)
                    failure["lr"] = hex(lr)
                    failure["plt_symbol_at_pc"] = getattr(probe, "plt", {}).get(pc)
            except BaseException as state_error:
                failure["pc_lr_read_error"] = repr(state_error)[:1000]
            with failure_path.open("x") as failure_file:
                failure_file.write(json.dumps(failure, indent=2, sort_keys=True) + "\n")
            raise
    finally:
        sys.argv = old_argv
        if old_path is None:
            os.environ.pop("PATH", None)
        else:
            os.environ["PATH"] = old_path
        if old_ld_library_path is None:
            os.environ.pop("LD_LIBRARY_PATH", None)
        else:
            os.environ["LD_LIBRARY_PATH"] = old_ld_library_path
    print(json.dumps({"written": str(destination.relative_to(ROOT)), "sha256": sha256(destination), "case": args.case}, sort_keys=True))


if __name__ == "__main__":
    main()
