#!/usr/bin/env python3
"""Replay omitted-color/null advanced-setting controls through native Drawing."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve()
ROOT = next(p for p in HERE.parents if (p / "Cargo.toml").is_file())
FIXTURE = ROOT / "conformance/magicpen-admission-defaults-native.json"
ADAPTER = ROOT / "conformance/magicpen_nocurve_source_events_native.py"
ADAPTER_SHA256 = "21f86bcc4011f81d6065e719f6ada0b2cfb64795129b0fde0854806a94c3c6df"
BASE_CALLER = ROOT / "conformance/magicpen_saved_caller_native.py"
BASE_CALLER_SHA256 = "cd2d613e202e35be261316a433533cda1c2f0ee76db150c4cca20963c80ba59c"
BOUNDARY_FIXTURE = ROOT / "conformance/magicpen-saved-caller-native.json"
BOUNDARY_FIXTURE_SHA256 = "05f8cf46ff64e3199205d7efae26a3a90c4ffe60649af544afaffef2467df453"
CASE = (
    "admission_default_controls",
    [(0, 0), (10, 0), (20, 2)],
    [0.3, 0.6, 1.0],
    [100, 110, 120],
    10,
    "set",
    False,
)
RUNTIME_PINS = {
    "python_executable": "889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9",
    "unicorn_package_init": "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b",
    "unicorn_python_module": "e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7",
    "unicorn_native_library": "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c",
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def runtime_closure() -> dict:
    sys.path.insert(0, str(ROOT / "scratch/apk-analysis-runtime/python"))
    import unicorn

    package = Path(unicorn.__file__).resolve().parent
    paths = {
        "python_executable": Path(sys.executable).resolve(),
        "unicorn_package_init": package / "__init__.py",
        "unicorn_python_module": package / "unicorn.py",
        "unicorn_native_library": package / "lib/libunicorn.so.2",
    }
    hashes = {key: sha256(path.read_bytes()) for key, path in paths.items()}
    if hashes != RUNTIME_PINS:
        raise RuntimeError(f"pinned Python/Unicorn runtime changed: {hashes}")
    if platform.python_version() != "3.13.5" or unicorn.__version__ != "2.1.4":
        raise RuntimeError("pinned Python/Unicorn version changed")
    return {
        "python": {"path": str(paths["python_executable"]), "version": platform.python_version(), "sha256": hashes["python_executable"]},
        "unicorn": {
            "version": unicorn.__version__,
            "package_path": str(package.relative_to(ROOT)),
            "init_sha256": hashes["unicorn_package_init"],
            "python_module_sha256": hashes["unicorn_python_module"],
            "native_library_path": str(paths["unicorn_native_library"].relative_to(ROOT)),
            "native_library_sha256": hashes["unicorn_native_library"],
        },
    }


def load_adapter():
    if sha256(ADAPTER.read_bytes()) != ADAPTER_SHA256:
        raise RuntimeError("pinned NoCurve source adapter changed")
    if sha256(BASE_CALLER.read_bytes()) != BASE_CALLER_SHA256:
        raise RuntimeError("pinned MagicPen saved-caller changed")
    if sha256(BOUNDARY_FIXTURE.read_bytes()) != BOUNDARY_FIXTURE_SHA256:
        raise RuntimeError("pinned advanced-setting boundary fixture changed")
    spec = importlib.util.spec_from_file_location("magicpen_nocurve_adapter", ADAPTER)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


def adapted_wrapper(module, output: Path) -> tuple[str, str]:
    wrapper, _ = module.adapted_wrapper(CASE, output)
    old_new = [
        (
            "for label,advanced_value,advanced_mode in [('unset',None,'omitted'),('null-arg',None,'null'),('empty','','value'),('v1','1;','value'),('v2','2;','value')]:",
            "for label,advanced_value,advanced_mode,color_mode in [('null-missing',None,'null','omit'),('null-black',None,'null','black'),('v1-missing','1;','value','omit'),('v1-black','1;','value','black')]:",
        ),
        (
            "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.MagicPen')])",
            "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.MagicPen')])\n if color_mode=='black':call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,0xff000000])",
        ),
        (
            "pen_set_calls=[]; add_calls=[]; redraws=[]; rt_buffer_snapshots=[]; pending_add_receivers=[]",
            "pen_set_calls=[]; add_calls=[]; redraws=[]; rt_buffer_snapshots=[]; pending_add_receivers=[]; color_set_calls=[]",
        ),
        (
            "set_size_addr=p.syms['SPen::Pen::SetSize(float)']; get_size_addr=p.syms['SPen::Pen::GetSize()']",
            "set_size_addr=p.syms['SPen::Pen::SetSize(float)']; get_size_addr=p.syms['SPen::Pen::GetSize()']; set_color_addr=p.syms['SPen::Pen::SetColor(int)']",
        ),
        (
            "  elif a==set_size_addr:",
            "  elif a==set_color_addr:color_set_calls.append({'this':hex(p.x(0)),'argb':f'0x{p.x(1)&0xffffffff:08x}'})\n  elif a==set_size_addr:",
        ),
        (
            " selected_pen=int(pen_set_calls[-1]['this'],16) if pen_set_calls else None\n size_bits=None",
            " selected_pen=int(pen_set_calls[-1]['this'],16) if pen_set_calls else None\n pen_color=None\n if selected_pen:call('SPen::Pen::GetColor()',[selected_pen]);pen_color=f'0x{p.x(0)&0xffffffff:08x}'\n size_bits=None",
        ),
        (
            "records.append({'case':label,",
            "records.append({'case':label,'color_mode':color_mode,'native_pen_color':pen_color,'native_pen_setcolor_calls':color_set_calls,",
        ),
        (
            "_expected={'unset':'GLV1','null-arg':'GLV1','empty':'GLV2','v1':'GLV1','v2':'GLV2'}[_row['case']]",
            "_expected={'null-missing':'GLV1','null-black':'GLV1','v1-missing':'GLV1','v1-black':'GLV1'}[_row['case']]",
        ),
        (
            "_expected_setting={'unset':(True,None),'null-arg':(True,None),'empty':(False,''),'v1':(False,'1;'),'v2':(False,'2;')}[_row['case']]",
            "_expected_setting={'null-missing':(True,None),'null-black':(True,None),'v1-missing':(False,'1;'),'v1-black':(False,'1;')}[_row['case']]",
        ),
    ]
    injection = ""
    for i, (old, new) in enumerate(old_new):
        injection += f"if source.count({old!r}) != 1: raise RuntimeError('adaptation anchor {i} changed')\n"
        injection += f"source=source.replace({old!r},{new!r},1)\n"
    marker = 'exec(compile(source, str(BASE), "exec"), {"__name__": "__main__", "__file__": str(Path(__file__).resolve())})'
    if wrapper.count(marker) != 1:
        raise RuntimeError("saved-caller execution anchor changed")
    wrapper = wrapper.replace(marker, injection + marker, 1)
    return wrapper, sha256(wrapper.replace(str(output), "<FIXTURE_OUTPUT>").encode())


def capture(module) -> dict:
    runtime = runtime_closure()
    with tempfile.TemporaryDirectory(prefix="sdocx-magicpen-defaults-", dir=ROOT / "tmp") as tmp:
        tmpdir = Path(tmp)
        output = tmpdir / "native.json"
        wrapper, wrapper_sha = adapted_wrapper(module, output)
        script = tmpdir / "caller.py"
        script.write_text(wrapper)
        env = os.environ.copy()
        env["PYTHONPATH"] = os.pathsep.join(
            [
                str(ROOT / "scratch/apk-analysis-runtime/python"),
                str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"),
                str(ROOT / "conformance"),
            ]
        )
        subprocess.run([sys.executable, str(script), "--write-fixture"], cwd=ROOT, env=env, check=True)
        raw = json.loads(output.read_text())
    rows = raw["cases"]
    expected_names = ["null-missing", "null-black", "v1-missing", "v1-black"]
    if [r["case"] for r in rows] != expected_names:
        raise RuntimeError("native case order or coverage changed")
    first = rows[0]
    for row in rows:
        if row["drawing_return"] != 1 or row["selected_drawable_class"] != "GLV1":
            raise RuntimeError(f"GLV1 Drawing admission failed: {row['case']}")
        if row["native_model_metadata"] != {
            "tool_type": 2,
            "stroke_type": 0,
            "curve_enabled": False,
            "color_argb": "0xff000000",
        }:
            raise RuntimeError(f"native ObjectStroke getter mismatch: {row['case']}")
        if row["native_pen_color"] != "0xff000000":
            raise RuntimeError(f"native Pen::GetColor mismatch: {row['case']}")
        if row["source_model_get_point_count"] != 3 or row["rtv_addpoint_record_count"] != 8:
            raise RuntimeError(f"source/output count mismatch: {row['case']}")
        vector = row["native_rtv1_vector"]
        vector_bytes = bytes.fromhex(vector["bytes_le_hex"])
        if len(vector_bytes) != vector["float_count"] * 4 or sha256(vector_bytes) != vector["sha256"]:
            raise RuntimeError(f"incomplete native vector: {row['case']}")
        if row["source_arrays"] != first["source_arrays"]:
            raise RuntimeError(f"source getter arrays differ: {row['case']}")
        if row["rtv_addpoint_argument_stream_hex_le"] != first["rtv_addpoint_argument_stream_hex_le"]:
            raise RuntimeError(f"complete AddPoint stream differs: {row['case']}")
        if vector["bytes_le_hex"] != first["native_rtv1_vector"]["bytes_le_hex"]:
            raise RuntimeError(f"complete post-return vector differs: {row['case']}")
        if len(row["native_pen_setcolor_calls"]) != 2 or any(
            call["argb"] != "0xff000000" for call in row["native_pen_setcolor_calls"]
        ):
            raise RuntimeError(f"native Pen setter color differs: {row['case']}")
    boundary = json.loads(BOUNDARY_FIXTURE.read_text())
    empty = next(row for row in boundary["cases"] if row["case"] == "empty")
    null = next(row for row in boundary["cases"] if row["case"] == "null-arg")
    v1 = next(row for row in boundary["cases"] if row["case"] == "v1")
    if (empty["selected_drawable_class"], empty["advanced_setting_getter"]) != (
        "GLV2",
        {"is_null": False, "utf8": ""},
    ):
        raise RuntimeError("empty advanced-setting boundary no longer selects GLV2")
    if null["selected_drawable_class"] != "GLV1" or v1["selected_drawable_class"] != "GLV1":
        raise RuntimeError("null/1; advanced-setting boundary changed")
    return {
        "schema": "sdocx-magicpen-admission-defaults-native-v1",
        "scope": "Synthetic ObjectStroke inputs through the pinned native Drawing caller, GLV1, and complete RTV1 streams. No SDK deserialization, real MagicPen document, texture binding, GPU draw, or raster proof.",
        "source_input": {
            "points_f32_u32_hex": ["00000000", "00000000", "41200000", "00000000", "41a00000", "40000000"],
            "pressure_f32_u32_hex": ["3e99999a", "3f19999a", "3f800000"],
            "timestamps_i32": [100, 110, 120],
            "size": 10.0,
            "curve_enabled": False,
            "tool_type": 2,
            "stroke_type": 0,
        },
        "provenance": {
            "runner_sha256": sha256(HERE.read_bytes()),
            "adapter_path": "conformance/magicpen_nocurve_source_events_native.py",
            "adapter_sha256": ADAPTER_SHA256,
            "base_caller_path": "conformance/magicpen_saved_caller_native.py",
            "base_caller_sha256": BASE_CALLER_SHA256,
            "boundary_fixture_path": "conformance/magicpen-saved-caller-native.json",
            "boundary_fixture_sha256": BOUNDARY_FIXTURE_SHA256,
            "wrapper_sha256_with_output_placeholder": wrapper_sha,
            "native_libraries": module.LIBRARIES,
            "native_helpers": module.HELPERS,
            "runtime_closure": runtime,
        },
        "cases": [
            {
                "case": row["case"],
                "advanced_mode": row["advanced_mode"],
                "advanced_setting_getter": row["advanced_setting_getter"],
                "color_mode": row["color_mode"],
                "model_getters": row["native_model_metadata"],
                "pen_get_color": row["native_pen_color"],
                "native_pen_set_color_calls": row["native_pen_setcolor_calls"],
                "selected_drawable_class": row["selected_drawable_class"],
                "drawing_return": row["drawing_return"],
                "source_arrays": row["source_arrays"],
                "source_model_get_point_count": row["source_model_get_point_count"],
                "addpoint_record_count": row["rtv_addpoint_record_count"],
                "addpoint_stream_sha256": row["rtv_addpoint_argument_stream_sha256"],
                "addpoint_stream_hex_le": row["rtv_addpoint_argument_stream_hex_le"],
                "post_return_vector_sha256": row["native_rtv1_vector"]["sha256"],
                "post_return_vector_bytes_le_hex": row["native_rtv1_vector"]["bytes_le_hex"],
                "redraw_entries": row["redraw_entries"],
            }
            for row in rows
        ],
        "conclusions": {
            "null_advanced_equals_literal_1_semicolon_for_selected_class_and_geometry": True,
            "omitted_color_equals_explicit_opaque_black_for_model_pen_getters_and_geometry": True,
            "empty_advanced_selects_glv2_and_must_remain_distinct": True,
            "raw_parser_none_is_preserved;_normalization_applies_only_at_native_admission": True,
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    module = load_adapter()
    result = capture(module)
    encoded = json.dumps(result, indent=2) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
    elif not FIXTURE.exists() or FIXTURE.read_text() != encoded:
        raise SystemExit("MagicPen admission-default fixture mismatch; use --write-fixture after review")
    print("verified four MagicPen omitted-default native Drawing controls")


if __name__ == "__main__":
    main()
