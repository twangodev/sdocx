"""Pinned native InkPen V4 size/tool admission controls.

Run from the repository with the existing native-analysis environment:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
  LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
    python3 conformance/inkpen_admission_native.py --check-fixture

Native constructors, getters, Drawing selection, MotionEvent accessors, V4
RedrawPen, and RTV1 AddPoint execute in pinned libraries. The IContext/canvas/
resource layouts and dispatch adapters are host-created, color conversion is
the caller's identity callback, and Bionic is not loaded as a system runtime.
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance/inkpen_saved_caller_native.py"
FIXTURE = ROOT / "conformance/inkpen-admission-native.json"
CALLER_SHA256 = "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04"
HELPER_SHA256 = {
    "oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
}
LIBRARY_SHA256 = {
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenDrawing.so": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "libSPenFountainPen.so": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "libSPenGraphics.so": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "libSPenInkPen.so": "939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401",
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenPenCommon.so": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}
CASES = {
    "size_omitted": (None, True, 1),
    "size_zero": (0.0, False, 1),
    "size_point_one": (0.1, False, 1),
    "size_ten": (10.0, False, 1),
    "size_thousand": (1000.0, False, 1),
    "tool_zero": (10.0, False, 0),
    "tool_two": (10.0, False, 2),
    "tool_three": (10.0, False, 3),
    "tool_four": (10.0, False, 4),
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def patch_source() -> tuple[str, list[dict[str, object]]]:
    if sha256(CALLER) != CALLER_SHA256:
        raise SystemExit("pinned InkPen saved caller source changed")
    source = CALLER.read_text(encoding="utf-8")
    patches: list[dict[str, object]] = []

    def replace_once(old: str, new: str, label: str) -> None:
        nonlocal source
        count = source.count(old)
        if count != 1:
            raise SystemExit(f"patch anchor {label!r}: expected one match, found {count}")
        source = source.replace(old, new, 1)
        patches.append({
            "label": label,
            "match_count": count,
            "old_sha256": hashlib.sha256(old.encode()).hexdigest(),
            "new_sha256": hashlib.sha256(new.encode()).hexdigest(),
        })

    replace_once(
        "parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')",
        "parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')\nparser.add_argument('--probe-size',type=float,default=10.0)\nparser.add_argument('--probe-tool',type=int,choices=(0,1,2,3,4),default=1)\nparser.add_argument('--omit-size-setter',action='store_true')",
        "add admission probe arguments",
    )
    replace_once(
        "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}",
        "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':args.probe_size,'curve_enabled':True,'tool_type':args.probe_tool,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}",
        "parameterize native Model size and tool setters",
    )
    replace_once(
        "p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]); p.call('SPen::ObjectStroke::GetPenSize() const',[obj]);",
        "if not args.omit_size_setter: p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);\n p.call('SPen::ObjectStroke::GetPenSize() const',[obj]);",
        "make native Model size setter omittable",
    )
    replace_once(
        "if setting_resolution != [args.width,args.height] or setting_size_bits != source_size_bits: raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')",
        "expected_clamped_size=struct.pack('<f',max(0.4,min(input_case['size'],800.0))).hex()\n if setting_resolution != [args.width,args.height] or setting_size_bits != expected_clamped_size: raise RuntimeError(f'native Pen settings mismatch: source={source_size_bits} expected_clamped={expected_clamped_size} actual={setting_size_bits} screen={setting_resolution}')",
        "assert native Pen size clamp",
    )
    replace_once(
        "result['motion_event_getter_returns']=native_motion_getter_returns",
        "result['motion_event_getter_returns']=native_motion_getter_returns\n source_tool_calls=[row for row in native_motion_getter_returns if row['signature']=='SPen::MotionEvent::GetToolType(int) const' and row['phase_entry']=='saved_redraw']\n if not source_tool_calls: raise RuntimeError('missing native saved MotionEvent GetToolType calls')\n terminal_event_ptr=int(source_tool_calls[-1]['args']['x0'],16)\n p.call('SPen::MotionEvent::GetSource() const',[terminal_event_ptr])\n result['native_motion_event_source_probe']={'method':'SPen::MotionEvent::GetSource() const','returned_source_u32':p.x(0)&0xffffffff}",
        "capture actual native terminal MotionEvent source getter",
    )
    return source, patches


def run_case(source: str, case_id: str, sentinel: tuple[float | None, bool, int], patches: list[dict[str, object]]) -> dict[str, object]:
    size, omitted, tool = sentinel
    argv = [str(CALLER), "--print-capture", "--advanced-setting", "empty", "--probe-size", str(0.0 if size is None else size), "--probe-tool", str(tool), "--width", "1920", "--height", "1080"]
    if omitted:
        argv.append("--omit-size-setter")
    previous_argv = sys.argv
    output = io.StringIO()
    sys.argv = argv
    try:
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(CALLER), "exec"), {"__file__": str(CALLER), "__name__": "__main__"})
    finally:
        sys.argv = previous_argv
    rendered = output.getvalue().strip()
    if not rendered.startswith("{"):
        raise RuntimeError(f"{case_id}: patched caller produced no JSON capture")
    row = json.loads(rendered)
    snapshots = row["native_calls"]["ObjectStroke_getter_snapshots"]
    if len(snapshots) < 2 or snapshots[0]["label"] != "before_drawObjectStroke":
        raise RuntimeError(f"{case_id}: missing native ObjectStroke getter snapshot")
    if row["route"]["selected_drawable_vtable_id"] != "v4":
        raise RuntimeError(f"{case_id}: expected empty-setting V4 route")
    if row["route"]["drawing_return"] != 1 or row["route"]["saved_redraw_return"] != 1:
        raise RuntimeError(f"{case_id}: Drawing or saved redraw failed")
    if not row["streams"]["saved_redraw"]["same_bytes_as_drawing_phase"]:
        raise RuntimeError(f"{case_id}: Drawing and saved RedrawPen streams differ")
    tool_returns = [int(item["return"]["x0"], 16) & 0xFFFFFFFF for item in row["motion_event_getter_returns"] if item["signature"] == "SPen::MotionEvent::GetToolType(int) const"]
    row["admission_probe"] = {
        "case_id": case_id,
        "requested_size": size,
        "size_setter_omitted": omitted,
        "requested_tool_type": tool,
        "native_model_getters_before_drawing": snapshots[0],
        "native_pen_setting_size_f32_bits": row["pen_setting_data"]["pen_size_f32_bits_offset_0"],
        "native_drawable_size_f32_bits": row["drawable_native_state"]["drawable_size_f32_bits"],
        "native_motion_event_get_tool_type_returns_u32": tool_returns,
        "native_motion_event_source_probe": row.get("native_motion_event_source_probe"),
        "host_boundaries": [
            "ObjectStroke size/tool setters and getters, Drawing factory selection, Pen size initialization, MotionEvent construction/getters, InkPen V4 RedrawPen, and RTV1 AddPoint execute in pinned native DSOs.",
            "Only requested size, optional size-setter omission, and tool type vary; source points, pressure, timestamps, screen dimensions, curve, stroke type, pen name, and empty V4 advanced setting are fixed.",
            "IContext/canvas/resource layouts and dispatch adapters are host-created; color conversion is the original caller's identity callback; Bionic is not loaded as a system runtime.",
        ],
    }
    return row


def capture(source: str, patches: list[dict[str, object]]) -> dict[str, object]:
    library_dir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    libraries = {name: sha256(library_dir / name) for name in LIBRARY_SHA256}
    helpers = {name: sha256(ROOT / "conformance" / name) for name in HELPER_SHA256}
    if libraries != LIBRARY_SHA256:
        raise SystemExit("one or more pinned native DSOs changed")
    if helpers != HELPER_SHA256:
        raise SystemExit("one or more pinned conformance helpers changed")
    source_sha = hashlib.sha256(source.encode()).hexdigest()
    cases = {}
    for case_id, sentinel in CASES.items():
        print(f"running {case_id}", file=sys.stderr, flush=True)
        cases[case_id] = run_case(source, case_id, sentinel, patches)
    return {
        "schema": "sdocx-inkpen-v4-size-tool-admission-v1",
        "original_caller_sha256": CALLER_SHA256,
        "patched_caller_sha256": source_sha,
        "patches": patches,
        "helper_sha256": helpers,
        "library_sha256": libraries,
        "profile": {"width": 1920, "height": 1080},
        "cases": cases,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    source, patches = patch_source()
    envelope = capture(source, patches)
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(envelope, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    else:
        actual = json.loads(FIXTURE.read_text())
        if actual != envelope:
            raise SystemExit("native InkPen size/tool admission fixture mismatch")
        print(f"verified all {len(envelope['cases'])} admission cases")


if __name__ == "__main__":
    main()
