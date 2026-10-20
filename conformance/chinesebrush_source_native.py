"""Replay ChineseBrush multi-event source-to-RTV1 vectors through Drawing."""
from pathlib import Path
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/chinesebrush_saved_caller_native.py"
BASE_SHA256 = "576846a4a060e3ee11eede9a3b9c626bcdb7b96a07d83a8305f119b628f3b27a"
FIXTURE = Path(__file__).with_name("chinesebrush-source-native.json")
RUNTIME_PINS = {
    "scratch/apk-analysis-runtime/python/unicorn/__init__.py": "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b",
    "scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/unicorn.py": "62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0",
    "scratch/apk-analysis-runtime/python/unicorn/arm64_const.py": "92f91a2b0a36cd34deee16db4f8da4ac0c9ae35c3ea1089296e5ef9d170a5be9",
    "scratch/apk-analysis-runtime/python/unicorn/lib/libunicorn.so.2": "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c",
    "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump": "b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb",
    "/usr/bin/nm": "ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1",
    "/usr/bin/c++filt": "7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2",
}

SOURCE = {
    "points": [[0.0, 0.0], [6.0, 3.0], [9.0, 12.0], [17.0, 13.0], [23.0, 15.0]],
    "size": 3.144,
    "tool_type": 2,
    "stroke_type": 0,
    "pressure": [0.3, 0.55, 0.75, 0.35, 0.95],
    "timestamps": [300, 311, 324, 338, 355],
    "tilt": [0.12, 0.32, 0.55, 0.22, 0.65],
    "orientation": [0.2, 0.35, 0.55, 0.18, 0.7],
}
CASES = {
    "curve-v1": ("one", True, 1),
    "curve-v2": ("two", True, 2),
    "curve-v3": ("three", True, 3),
    "line-v2": ("two", False, 2),
    "line-v3": ("three", False, 3),
    "curveoff5-v2": ("two", False, 2),
    "curveoff5-v3": ("three", False, 3),
}


def three_source(points, pressures, timestamps, *, size=3.144, tool=2, curve=True):
    count = len(points)
    return {
        "points": points,
        "size": size,
        "tool_type": tool,
        "stroke_type": 0,
        "pressure": pressures,
        "timestamps": timestamps,
        "tilt": [0.12 + i * 0.01 for i in range(count)],
        "orientation": [0.2 + i * 0.02 for i in range(count)],
        "curve_enabled": curve,
    }


BASE3 = three_source([[1.0, -2.0], [3.0, 1.0], [6.0, 4.0]], [0.3, 0.6, 0.85], [300, 315, 340])
SLOW3 = three_source([[1.0, -2.0], [6.0, 4.0], [10.0, 8.0]], [0.3, 0.6, 0.85], [300, 30000, 60000])
FAST3 = dict(SLOW3, timestamps=[300, 301, 302])
WIDE3 = dict(SLOW3, size=8.333333)
WIDER3 = dict(SLOW3, size=10.0)
THREE_CASES = {}


def add_three(name, selector, version, spec, width=1920, height=1080):
    THREE_CASES[name] = (selector, spec, width, height, version)


for selector, version in (("one", 1), ("two", 2), ("three", 3)):
    add_three(f"three-baseline-v{version}", selector, version, BASE3)
    add_three(f"three-pressure-v{version}", selector, version, dict(BASE3, pressure=[0.05, 0.4, 0.9]))
    add_three(f"three-tool1-v{version}", selector, version, dict(BASE3, tool_type=1))
    add_three(f"three-timeshift-v{version}", selector, version, dict(BASE3, timestamps=[1300, 1315, 1340]))
    add_three(f"three-curveoff-v{version}", selector, version, dict(BASE3, curve_enabled=False))
    add_three(f"three-repeat-floor5-v{version}", selector, version, WIDE3)
    add_three(f"three-repeat-floor6-v{version}", selector, version, WIDER3)
for selector, version in (("two", 2), ("three", 3)):
    add_three(f"three-display-portrait-v{version}", selector, version, BASE3, 1440, 2560)
    add_three(f"three-slow-v{version}", selector, version, SLOW3)
    add_three(f"three-fast-v{version}", selector, version, FAST3)
    add_three(f"three-slow-portrait-v{version}", selector, version, SLOW3, 1440, 2560)
    add_three(f"three-uneven-early-v{version}", selector, version, dict(BASE3, points=SLOW3["points"], timestamps=[300, 330, 340]))
    add_three(f"three-uneven-late-v{version}", selector, version, dict(BASE3, points=SLOW3["points"], timestamps=[300, 310, 340]))
for selector, version in (("one", 1), ("three", 3)):
    add_three(f"three-duplicate-v{version}", selector, version, dict(BASE3, points=[[1.0, -2.0], [1.0, -2.0], [6.0, 4.0]]))
    add_three(f"three-micro-v{version}", selector, version, dict(BASE3, points=[[1.0, -2.0], [1.1, -2.1], [1.2, -2.2]]))

SOURCE_CASES = {
    name: (selector, dict(SOURCE, curve_enabled=curve), 1920, 1080, version)
    for name, (selector, curve, version) in CASES.items()
}
SOURCE_CASES.update(THREE_CASES)

# Eight additional frozen source cases exercise arbitrary event counts, tool-1
# routing, and the selected native Pen size clamp.
EXTRA_POINTS = [[0.0, 0.0], [6.0, 3.0], [9.0, 12.0], [17.0, 13.0], [23.0, 15.0], [31.0, 18.0], [36.0, 28.0], [45.0, 34.0]]
EXTRA_PRESSURES = [0.3, 0.55, 0.75, 0.35, 0.95, 0.5, 0.7, 0.4]
EXTRA_TIMES = [300, 311, 324, 338, 355, 376, 390, 407]
for _name, _selector, _version, _count, _size, _tool in (
    ("proto2-v1", "one", 1, 2, 3.144, 2),
    ("proto4-v2", "two", 2, 4, 3.144, 2),
    ("proto6-v3", "three", 3, 6, 3.144, 2),
    ("proto7-v2", "two", 2, 7, 3.144, 2),
    ("proto8-v3", "three", 3, 8, 3.144, 2),
    ("long-tool1-v3", "three", 3, 8, 3.144, 1),
    ("size-low-v2", "two", 2, 5, 0.1, 2),
    ("size-high-v3", "three", 3, 5, 900.0, 2),
):
    SOURCE_CASES[_name] = (
        _selector,
        three_source(EXTRA_POINTS[:_count], EXTRA_PRESSURES[:_count], EXTRA_TIMES[:_count], size=_size, tool=_tool, curve=True),
        1920,
        1080,
        _version,
    )


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(case_id):
    if sha(BASE) != BASE_SHA256:
        raise SystemExit("pinned ChineseBrush caller adapter changed")
    selector, spec, display_width, display_height, expected_version = SOURCE_CASES[case_id]
    spec = dict(spec)
    source = BASE.read_text()
    anchor = "source = CALLER.read_text()"
    if source.count(anchor) != 1:
        raise RuntimeError("ChineseBrush adapter source anchor changed")
    inject = (
        anchor
        + "\nif 'CHINESE_BRUSH_NATIVE_SPEC' in os.environ:\n"
        + " import ast\n"
        + " _source_spec=ast.literal_eval(os.environ['CHINESE_BRUSH_NATIVE_SPEC'])\n"
        + " _source_anchor=\"input_case={'points':[[1.,-2.],[3.,1.],[6.,4.]],'size':3.144,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[.3,.6,.85],'timestamps':[300,315,340],'tilt':[.12,.32,.55],'orientation':[.2,.35,.55]}\"\n"
        + " source=replace_once(source,_source_anchor,'input_case='+repr(_source_spec))\n"
        + " source=replace_once(source,\"'source_arrays':source_object_arrays}\",\"'source_arrays':source_object_arrays,'native_object_stroke_snapshots':object_stroke_snapshots}\")\n"
        + " _resolution_guard=\"  raise RuntimeError('constructor settings snapshot unexpectedly already equals post-Drawing resolution')\"\n"
        + " if source.count(_resolution_guard)!=1: raise RuntimeError('display resolution guard patch anchor changed')\n"
        + " source=source.replace(_resolution_guard,\"  pass # Profile call evidence is retained even when constructor values already match.\")\n"
    )
    source = source.replace(anchor, inject, 1)
    profile_anchor = 'sys.argv = [str(CALLER), "--print-capture"] + (["--omit-advanced-setting"] if _capture_setting is None else ["--advanced-setting", _capture_setting])'
    profile_patch = profile_anchor + '\nsys.argv += ["--width", os.environ["CHINESE_BRUSH_NATIVE_WIDTH"], "--height", os.environ["CHINESE_BRUSH_NATIVE_HEIGHT"]]'
    if source.count(profile_anchor) != 1:
        raise RuntimeError("ChineseBrush profile argument anchor changed")
    source = source.replace(profile_anchor, profile_patch, 1)
    check_start = source.index('source = replace_once(source, " if len(stream) != 1144')
    check_end = source.index(
        'source = replace_once(source,\n    "\'runtime_normalization_contract\'',
        check_start,
    )
    output_check = source[check_start:check_end]
    expected = '" if len(stream) != 80 or len(chinesebrush_buffer_events) != 5:'
    if output_check.count(expected) != 1:
        raise RuntimeError("ChineseBrush output-count patch anchor changed")
    output_check = output_check.replace(
        expected,
        '" if len(stream) == 0 or len(chinesebrush_buffer_events) == 0:',
        1,
    )
    source = source[:check_start] + output_check + source[check_end:]
    float_line = next(
        line
        for line in source.splitlines()
        if line.startswith('source = replace_once(source, "\'float_args\':')
    )
    float_rewrite = '''source = replace_once(source, "'float_args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(4)]", "'float_args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(5)],'vector4_arg_bytes':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_X1),16)).hex(),'scalar_s0_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()")
source = replace_once(source, "'float_args':r['float_args']", "'float_args':r['float_args'],'vector4_arg_bytes':r['vector4_arg_bytes'],'scalar_s0_f32_bits':r['scalar_s0_f32_bits']")'''
    source = source.replace(float_line, float_rewrite, 1)
    os.environ["CHINESE_BRUSH_NATIVE_SPEC"] = repr(spec)
    os.environ["CHINESE_BRUSH_NATIVE_WIDTH"] = str(display_width)
    os.environ["CHINESE_BRUSH_NATIVE_HEIGHT"] = str(display_height)
    sys.argv = [str(BASE), "--_capture", selector]
    namespace = {"__name__": "__main__", "__file__": str(BASE), "__package__": None}
    exec(compile(source, str(BASE), "exec"), namespace)


def check_native_input(case_id, value, spec, selector):
    """Join declared input to executed Model getters, never copied metadata alone."""
    def require(condition, field):
        if not condition:
            raise RuntimeError(f"{case_id}: native {field} differs from declared input")

    count = len(spec["points"])
    source = value["input"]
    channels = {
        "points": (8, "f", [v for point in spec["points"] for v in point]),
        "pressures": (4, "f", spec["pressure"]),
        "timestamps": (4, "i", spec["timestamps"]),
        "tilts": (4, "f", spec["tilt"]),
        "orientations": (4, "f", spec["orientation"]),
    }
    for name, (element_size, encoding, values) in channels.items():
        require(len(values) == count * (2 if name == "points" else 1), name + " input extent")
        expected = {
            "count": count,
            "element_size": element_size,
            "data_hex": struct.pack("<" + encoding * len(values), *values).hex(),
        }
        require(source["source_arrays"][name] == expected, name + " getter bytes/count/element size")

    size_bits = struct.pack("<f", spec["size"]).hex()
    setting = {"one": "1;", "two": "2;", "three": "3;"}[selector]
    model = source["native_model_getter_snapshot"]
    require(model == {
        "pen_name": "com.samsung.android.sdk.pen.pen.preload.ChineseBrush",
        "advanced_setting": setting,
        "advanced_setting_getter_called": True,
        "size": struct.unpack("<f", bytes.fromhex(size_bits))[0],
        "size_level": 30,
        "color_argb_u32": "0xff6cdd8f",
        "tool_type": spec["tool_type"],
    }, "Model metadata getters")
    snapshots = source["native_object_stroke_snapshots"]
    require(len(snapshots) == 1, "ObjectStroke snapshot count")
    snapshot = snapshots[0]
    for field, expected in {
        "label": "before_drawObjectStroke", "size_bits": size_bits,
        "point_count": count, "curve_enabled": int(spec["curve_enabled"]),
        "tool_type": spec["tool_type"], "stroke_type": spec["stroke_type"],
    }.items():
        require(snapshot[field] == expected, field + " getter")
    require(source["size_f32_bits"] == size_bits, "source size bits")
    effective_size = min(800.0, max(0.4, struct.unpack("<f", bytes.fromhex(size_bits))[0]))
    require(value["native_selection"]["size_f32_bits"] == struct.pack("<f", effective_size).hex(), "selected Pen size")

    producer = value["producer"]
    rows = producer["append_rows"]
    require(len(rows) == producer["callback_count"], "callback count")
    for i, row in enumerate(rows):
        require(row["before_count_bytes"] == i * 16 and row["after_count_bytes"] == (i + 1) * 16, "append offsets")
        delta = bytes.fromhex(row["delta_bytes"])
        vector = bytes.fromhex(row["vector4_arg_bytes"])
        require(len(delta) == 16, "native append record is exactly four float32 lanes / 16 bytes")
        require(len(row["float_args"]) == 5, "diagnostic FP register snapshot")
        require(len(vector) == 16, "Vector4 callback input is exactly four float32 lanes")
        require(vector[:12] == delta[:12], "XYZ callback input-to-record join")
        scalar = bytes.fromhex(row["scalar_s0_f32_bits"])
        require(len(scalar) == 4, "separate scalar callback input is one float32")
        require(row["float_args"][0] == f"0x{int.from_bytes(scalar, 'little'):08x}", "scalar callback input register")
    raw = bytes.fromhex(producer["buffer_hex"])
    require(raw == b"".join(bytes.fromhex(row["delta_bytes"]) for row in rows), "complete append stream")
    require(len(raw) == producer["buffer_bytes"] == len(rows) * 16, "buffer extent")
    require(hashlib.sha256(raw).hexdigest() == producer["buffer_sha256"], "buffer hash")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--_capture", choices=tuple(SOURCE_CASES))
    args = parser.parse_args()
    # Validate the declared project-local runtime before importing its modules.
    for path, expected in RUNTIME_PINS.items():
        if sha(ROOT / path) != expected:
            raise SystemExit(f"ChineseBrush runtime/tool pin changed: {path}")
    for tool in ("nm", "c++filt"):
        selected = shutil.which(tool)
        if selected is None or Path(selected).resolve() != Path("/usr/bin", tool).resolve():
            raise SystemExit(f"Use the pinned /usr/bin/{tool} on PATH")
    unicorn_spec = importlib.util.find_spec("unicorn")
    expected_origin = ROOT / "scratch/apk-analysis-runtime/python/unicorn/__init__.py"
    if unicorn_spec is None or Path(unicorn_spec.origin).resolve() != expected_origin.resolve():
        raise SystemExit("Use PYTHONPATH=scratch/apk-analysis-runtime/python for the pinned Unicorn runtime")
    if args._capture:
        capture(args._capture)
        return

    if sha(BASE) != BASE_SHA256:
        raise SystemExit("pinned ChineseBrush caller adapter changed")
    captures = {}
    index = []
    def capture_case(case_id):
        selector, spec, display_width, display_height, version = SOURCE_CASES[case_id]
        result = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--_capture", case_id],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
        )
        value = json.loads(result.stdout)
        check_native_input(case_id, value, spec, selector)
        assert value["input"]["curve_enabled"] is spec["curve_enabled"], case_id
        assert value["input"]["source_arrays"]["points"]["count"] == len(spec["points"]), case_id
        assert value["native_selection"]["version"] == version, case_id
        assert value["profile"]["width"] == display_width and value["profile"]["height"] == display_height, case_id
        assert len(value["producer"]["append_rows"]) == value["producer"]["callback_count"], case_id
        assert all(len(row["float_args"]) == 5 for row in value["producer"]["append_rows"]), case_id
        return case_id, version, spec, value

    with ThreadPoolExecutor(max_workers=2) as pool:
        for case_id, version, spec, value in pool.map(capture_case, SOURCE_CASES):
            captures[case_id] = value
            index.append(
                {
                    "id": case_id,
                    "version": version,
                    "curve_enabled": spec["curve_enabled"],
                    "callback_count": value["producer"]["callback_count"],
                    "stream_sha256": value["producer"]["buffer_sha256"],
                }
            )
    fixture = {
        "schema": "chinesebrush-source-native-v1",
        "provenance": {
            "runner_sha256": sha(Path(__file__)),
            "base_adapter": "chinesebrush_saved_caller_native.py",
            "base_adapter_sha256": BASE_SHA256,
            "runtime_tool_preflight_sha256": RUNTIME_PINS,
        },
        "source": SOURCE,
        "cases": captures,
        "case_index": index,
        "boundary": "Synthetic three-, five-, and two/four/six/seven/eight-event ObjectStroke sources; pinned native Model getters, ObjectDrawing version dispatch, selected ChineseBrush GLV1/2/3 redraw, and RTV1 AddPoint execute. No app-deserialized stroke, renderer, GLES, or PDF claim.",
    }
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(captures)} cases)")
    elif not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != fixture:
        raise SystemExit("ChineseBrush source fixture mismatch; use --write-fixture to intentionally refresh")
    else:
        print(f"verified {len(captures)} ChineseBrush source cases")


if __name__ == "__main__":
    main()
