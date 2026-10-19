"""Reproduce joined InkPen2 RTV3 Draw matrix scale controls under Mesa.

Run with the project's pinned runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    python3 conformance/inkpen2_joined_scale_native.py --check-fixture
"""
import argparse
import base64
import hashlib
import json
import pathlib
import subprocess
import sys
import zlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
RUNNER = ROOT / "conformance/inkpen2_joined_paint_native.py"
FIXTURE = ROOT / "conformance/inkpen2-joined-scale-native.json"


def capture(scale, color_argb="0xff336699"):
    command = [
        sys.executable, str(RUNNER), "--print-capture",
        "--matrix-scale", str(scale),
        "--matrix-translate-x", "64", "--matrix-translate-y", "48",
        "--output-width", "256", "--output-height", "192",
        "--color-argb", color_argb,
    ]
    completed = subprocess.run(command, cwd=ROOT, check=True, text=True, capture_output=True)
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"joined native runner did not emit JSON: {completed.stdout[-1000:]!r}") from error
    if completed.stderr.strip():
        raise RuntimeError(f"joined native runner wrote unexpected stderr: {completed.stderr[-1000:]!r}")

    paint = result["joined_native_paint"]["paint"]
    readback = paint["readback"]
    producer = result["joined_native_paint"]["producer"]
    if producer["byte_count"] != 900 or len(bytes.fromhex(producer["native_AddPoint_vector_bytes_hex"])) != 900:
        raise RuntimeError("expected the actual Drawing-selected V9 receiver's 900-byte producer stream")
    if producer["matching_upload_bytes_hex"] != producer["native_AddPoint_vector_bytes_hex"]:
        raise RuntimeError("uploaded buffer differs from the caller-produced V9 bytes")
    if not result["joined_native_paint"]["receiver"]["receiver_is_selected_drawing_rt"]:
        raise RuntimeError("paint receiver was not the Drawing-selected RT")
    if readback["attachment"]["texture_level0"] != {"width": 256, "height": 192, "internal_format": 0x8058}:
        raise RuntimeError("output attachment is not the asserted 256x192 RGBA8 texture")
    if not readback["attachment"]["framebuffer_complete"] or paint["gl_error_after_draw"] != 0 or paint["gl_errors_recorded"]:
        raise RuntimeError("native Draw target or GLES error checks failed")
    if readback["baseline_all_zero"] is not True:
        raise RuntimeError("scale target was not zeroed before native Draw")

    raw_rgba = zlib.decompress(base64.b64decode(readback["full_bytes_zlib_base64"]))
    if len(raw_rgba) != 256 * 192 * 4 or hashlib.sha256(raw_rgba).hexdigest() != readback["sha256"]:
        raise RuntimeError("RGBA readback length/hash mismatch")
    uniform_calls = readback["actual_uniform_matrix_calls"]
    if len(uniform_calls) != 1 or uniform_calls[0]["program"] != 5:
        raise RuntimeError(f"expected one captured program-5 matrix uniform call, got {uniform_calls!r}")
    native_draws = readback.get("actual_instanced_draw_state", [])
    if len(native_draws) != 1 or native_draws[0]["program"] != 5 or native_draws[0]["args"] != [5, 0, 4, 25]:
        raise RuntimeError(f"expected one actual program-5 instanced draw state, got {native_draws!r}")
    if native_draws[0]["viewport"] != [0, 0, 256, 192]:
        raise RuntimeError(f"unexpected viewport at instanced draw: {native_draws!r}")
    attachment = readback["attachment"]
    if native_draws[0]["draw_framebuffer"] != attachment["framebuffer"] or native_draws[0]["read_framebuffer"] != attachment["framebuffer"]:
        raise RuntimeError(f"draw-time framebuffer differs from the readback attachment: {native_draws!r}")
    branch = readback.get("draw_rt_plus_68_branch_trace", [])
    if readback.get("rt_plus_68_u8_before_draw") != "00" or len(branch) != 2:
        raise RuntimeError("missing expected zero pre-Draw RT+68 state and branch trace")
    entry = int(readback["draw_symbol_entry_pc"], 16)
    if branch[0]["pc"] != hex(entry + 0x68) or branch[0]["instruction_bytes_hex"] != "88124139a8150034":
        raise RuntimeError(f"native Draw+0x68 load/conditional branch mismatch: {branch!r}")
    if branch[0]["rt_plus_68_u8"] != 0 or branch[1]["pc"] != hex(entry + 0x320) or branch[1]["branch_path"] != "default":
        raise RuntimeError(f"native Draw did not take the observed RT+68 zero branch: {branch!r}")

    return {
        "draw_scale": scale,
        "source_argb_u32": result["input"]["color_argb_u32"],
        "draw_matrix_f32_row_major": readback["draw_matrix_f32_row_major"],
        "actual_projection_uniform_calls": uniform_calls,
        "actual_viewport_calls": readback["actual_viewport_calls"],
        "receiver": result["joined_native_paint"]["receiver"],
        "native_init_blend_descriptor": result["joined_native_paint"]["native_init_blend_descriptor"],
        "producer": producer,
        "paint": {
            "draw_pass": paint["rtv3_draw_pass"],
            "target_setup": paint["target_setup"],
            "readback": readback,
        },
        "libraries_sha256": result["libraries"],
        "support_helpers_sha256": result["support_helpers_sha256"],
        "native_scope": result["scope"],
    }


def build(case):
    scales = (case,) if case else (1, 2, 4)
    captures = {f"scale_{scale}x": capture(scale) for scale in scales}
    first = next(iter(captures.values()))
    for name, row in captures.items():
        if row["producer"] != first["producer"]:
            raise RuntimeError(f"{name} producer bytes or upload evidence differ across Draw matrices")
        if row["libraries_sha256"] != first["libraries_sha256"] or row["support_helpers_sha256"] != first["support_helpers_sha256"]:
            raise RuntimeError(f"{name} dependency pins differ across captures")
        if row["receiver"]["drawing_selected_rt_identity"] != first["receiver"]["drawing_selected_rt_identity"]:
            raise RuntimeError(f"{name} did not retain the same emulator-native receiver identity")
    partial_alpha = capture(1, "0x80336699") if not case else None
    return {
        "schema": "sdocx-inkpen2-joined-scale-native-v1",
        "capture_boundary": [
            "Actual synthetic ObjectStroke-to-Drawing factory path selects the V9 drawable and produces the vector records.",
            "Each scale capture uses that same selected native RT and uploads the complete caller-produced byte stream before RTV3 Draw.",
            "Queue execution, Mesa GLES context, explicit screen profile, output RGBA bitmap/canvas, and matrix are host supplied.",
            "This records a bounded native Draw under a supplied matrix; it does not establish device defaults, the application canvas lifecycle, final composition, or export behavior.",
            "Pixel support and bytes are native Mesa observations, not a claim of exact analytic-vector equivalence.",
        ],
        "source": {
            "case": first["native_scope"],
            "profile": {"width": 1440, "height": 2560},
            "advanced_setting": "9;",
            "producer_sha256": first["producer"]["sha256"],
            "producer_byte_count": first["producer"]["byte_count"],
            "native_libraries_sha256": first["libraries_sha256"],
            "support_helpers_sha256": first["support_helpers_sha256"],
        },
        "cases": captures,
        "partial_alpha_control": partial_alpha,
    }


parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group(required=True)
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
parser.add_argument("--case", type=int, choices=(1, 2, 4), help="capture just one scale for diagnosis; full fixture mode uses all scales")
args = parser.parse_args()
if args.case and args.write_fixture:
    raise SystemExit("--write-fixture always regenerates the complete 1x/2x/4x fixture")
result = build(args.case)
encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
if args.write_fixture:
    FIXTURE.write_text(encoded)
    print(f"wrote {FIXTURE.relative_to(ROOT)}")
else:
    if args.case:
        print(json.dumps(result, indent=2, sort_keys=True))
        raise SystemExit(0)
    if not FIXTURE.exists():
        raise SystemExit(f"missing fixture: {FIXTURE.relative_to(ROOT)}")
    expected = json.loads(FIXTURE.read_text())
    if expected != result:
        raise SystemExit("native joined scale fixture mismatch")
    print(f"verified {FIXTURE.relative_to(ROOT)}")
