"""Hash-pinned InkPen V4 system-display profile matrix.

Run with the project's local native runtime and toolchain:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
  LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
    python3 conformance/inkpen_display_profiles_native.py --check-fixture

Five positive display profiles exercise the native Drawing setter and getter.
The zero and wider-than-u16 cases are ABI controls only, not physical-profile
defaults. The existing InkPen caller remains unmodified; those two controls
execute its source after exact-count, path-independent source patches.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance" / "inkpen_saved_caller_native.py"
FIXTURE = ROOT / "conformance" / "inkpen-display-profiles-native.json"

CALLER_SHA256 = "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04"
HELPER_SHA256 = {
    "elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
}
LIBRARY_SHA256 = {
    "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "Composer": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "Drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "Graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "InkPen": "939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401",
    "Model": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}

POSITIVE_CASES = (
    ("profile_1920x1080", 1920, 1080),
    ("profile_1440x2560", 1440, 2560),
    ("profile_2560x1440", 2560, 1440),
    ("profile_1080x1920", 1080, 1920),
    ("profile_2048x1536", 2048, 1536),
)
ABI_CASES = (
    ("abi_control_zero_width_1080", 0, 1080, "zero-width"),
    ("abi_control_70000x80000", 70000, 80000, "wide-int32"),
)

WIDTH_GUARD = "if not (0 < args.width < 65536 and 0 < args.height < 65536): raise SystemExit('profile dimensions must be positive u16 values')"
MOV_HELPER = """def mov_w0_ret(value):
 if not 0 <= value <= 0xffff: raise ValueError('screen dimension exceeds MOVZ immediate')
 return struct.pack('<II',0x52800000|(value<<5),0xd65f03c0)"""
MOV_HELPER_WIDE = """def mov_w0_ret(value):
 if not 0 <= value <= 0xffffffff: raise ValueError('screen dimension exceeds 32-bit W register')
 low = 0x52800000 | ((value & 0xffff) << 5)
 high = 0x72a00000 | (((value >> 16) & 0xffff) << 5)
 return struct.pack('<III',low,high,0xd65f03c0)"""
BOOTSTRAP = """import pathlib,sys
caller=pathlib.Path(sys.argv[1]).resolve()
sys.argv=[str(caller),*sys.argv[2:]]
namespace={'__file__':str(caller),'__name__':'__main__'}
exec(compile(sys.stdin.read(),str(caller),'exec'),namespace)
"""


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def patch_for_abi_controls(source: bytes) -> tuple[bytes, list[dict[str, object]]]:
    text = source.decode("utf-8")
    anchors: list[dict[str, object]] = []
    replacements = (
        (
            "dimension-guard-u16",
            WIDTH_GUARD,
            "dimension-guard-int32-pattern",
            "if not (0 <= args.width <= 0xffffffff and 0 <= args.height <= 0xffffffff): raise SystemExit('profile controls must fit W-register bit patterns')",
        ),
        ("movz-only-w0-return", MOV_HELPER, "movz-movk-w0-return", MOV_HELPER_WIDE),
    )
    for anchor_name, old, replacement_name, new in replacements:
        count = text.count(old)
        if count != 1:
            raise RuntimeError(f"patch anchor {anchor_name!r} expected exactly once, found {count}")
        text = text.replace(old, new, 1)
        anchors.append(
            {
                "anchor": anchor_name,
                "match_count": count,
                "replacement": replacement_name,
            }
        )
    patched = text.encode("utf-8")
    # The runner's actual checkout is passed via __file__, not embedded into
    # source bytes. This guard fails if a future bootstrap starts baking paths
    # into the executed caller and makes its hash checkout-dependent.
    for path in (str(ROOT), str(CALLER)):
        if path.encode("utf-8") in patched:
            raise RuntimeError(f"absolute checkout path was embedded in patched source: {path}")
    return patched, anchors


def capture(name: str, width: int, height: int, kind: str, abi_patch: str | None = None) -> dict[str, object]:
    caller_source = CALLER.read_bytes()
    if sha256(caller_source) != CALLER_SHA256:
        raise RuntimeError("committed InkPen caller source hash changed")
    patch_anchors: list[dict[str, object]] = []
    executed_source = caller_source
    if abi_patch is not None:
        executed_source, patch_anchors = patch_for_abi_controls(caller_source)
    executed_hash = sha256(executed_source)
    env = os.environ.copy()
    runtime_python = ROOT / "scratch" / "apk-analysis-runtime" / "python"
    env["PYTHONPATH"] = os.pathsep.join(
        (str(runtime_python), str(ROOT / "conformance"), env.get("PYTHONPATH", ""))
    ).rstrip(os.pathsep)
    arguments = [
        "--print-capture",
        "--advanced-setting",
        "empty",
        "--width",
        str(width),
        "--height",
        str(height),
    ]
    if abi_patch is None:
        result = subprocess.run(
            [sys.executable, str(CALLER), *arguments],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            check=True,
        )
    else:
        result = subprocess.run(
            [sys.executable, "-c", BOOTSTRAP, str(CALLER), *arguments],
            cwd=ROOT,
            env=env,
            input=executed_source.decode("utf-8"),
            text=True,
            capture_output=True,
            check=True,
        )
    case = json.loads(result.stdout)
    validate_capture(case, name, width, height)
    case["profile_case"] = {
        "name": name,
        "kind": kind,
        "system_display_width_px": width,
        "system_display_height_px": height,
    }
    case["executed_source"] = {
        "original_caller_sha256": sha256(caller_source),
        "executed_caller_source_sha256": executed_hash,
        "patch_kind": abi_patch,
        "patch_anchors": patch_anchors,
        "__file__": "conformance/inkpen_saved_caller_native.py",
        "absolute_checkout_path_in_source": False,
    }
    return case


def validate_capture(case: dict[str, object], name: str, width: int, height: int) -> None:
    if case["schema"] != "inkpen-saved-caller-native-v1":
        raise RuntimeError(f"{name}: unexpected native capture schema")
    if case["input"]["advanced_setting_state"] != "empty":
        raise RuntimeError(f"{name}: expected V4 empty-setting input")
    if case["route"]["selected_drawable_vtable_id"] != "v4":
        raise RuntimeError(f"{name}: Drawing did not select InkPen V4")
    if case["route"]["drawing_return"] != 1 or case["route"]["saved_redraw_return"] != 1:
        raise RuntimeError(f"{name}: Drawing/saved redraw failed")
    if case["screen_resolution"] != [{"width": width, "height": height}]:
        raise RuntimeError(f"{name}: unexpected IContext setter arguments")
    setting = case["pen_setting_data"]["screen_resolution_i32_offsets_32_36"]
    if setting != [width, height]:
        raise RuntimeError(f"{name}: native Pen setting getter differs from input")
    streams = case["streams"]
    drawing = streams["draw_object_stroke"]
    saved = streams["saved_redraw"]
    if drawing["callback_count"] != len(drawing["values_f32_bits"]) // 3:
        raise RuntimeError(f"{name}: Drawing callback count disagrees with full triples")
    if drawing["float_count"] != len(drawing["values_f32_bits"]):
        raise RuntimeError(f"{name}: Drawing float count disagrees with full vector")
    if not saved["same_bytes_as_drawing_phase"] or saved["sha256"] != drawing["sha256"]:
        raise RuntimeError(f"{name}: Drawing and saved redraw callback streams differ")
    if len(case["native_calls"]["source_ObjectStroke_arrays"]["points"]["data_hex"]) == 0:
        raise RuntimeError(f"{name}: source ObjectStroke arrays missing")
    helpers = case["helper_hashes"]
    expected_helpers = {
        **HELPER_SHA256,
        "inkpen_saved_caller_native.py": CALLER_SHA256,
    }
    if helpers != expected_helpers:
        raise RuntimeError(f"{name}: helper hash set changed: {helpers}")
    if case["libraries"] != LIBRARY_SHA256:
        raise RuntimeError(f"{name}: native DSO hash set changed")


def generate() -> dict[str, object]:
    caller_source = CALLER.read_bytes()
    if sha256(caller_source) != CALLER_SHA256:
        raise RuntimeError("committed InkPen caller source hash changed")
    cases: dict[str, object] = {}
    for name, width, height in POSITIVE_CASES:
        print(f"capturing {name} ({width}x{height})", file=sys.stderr, flush=True)
        cases[name] = capture(name, width, height, "physical-display-profile")
    for name, width, height, patch_kind in ABI_CASES:
        print(f"capturing {name} ({width}x{height})", file=sys.stderr, flush=True)
        cases[name] = capture(name, width, height, "abi-test-control", patch_kind)
    return {
        "schema": "sdocx-inkpen-display-profiles-native-v1",
        "generator_sha256": sha256(Path(__file__).read_bytes()),
        "caller_path": "conformance/inkpen_saved_caller_native.py",
        "caller_sha256": CALLER_SHA256,
        "helper_sha256": HELPER_SHA256,
        "library_sha256": LIBRARY_SHA256,
        "cases": cases,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    generated = generate()
    encoded = json.dumps(generated, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0
    if not FIXTURE.exists():
        raise SystemExit("InkPen display-profile fixture missing")
    actual = json.loads(FIXTURE.read_text())
    expected = json.loads(encoded)
    if actual != expected:
        raise SystemExit("InkPen display-profile native fixture mismatch")
    print("verified five display profiles and two int32 ABI controls; all native streams and pins match")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
