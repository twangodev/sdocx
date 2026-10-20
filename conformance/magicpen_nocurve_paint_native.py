#!/usr/bin/env python3
"""Strict six-case replay for the MagicPen NoCurve native paint witness."""

from __future__ import annotations

import argparse
import ast
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zlib


def discover_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if parent.name == "conformance":
            return parent.parent
        if (parent / "conformance").is_dir() and (parent / "AGENTS.md").is_file():
            return parent
    raise RuntimeError("could not locate repository from a conformance-parent path")


ROOT = discover_root()
HERE = Path(__file__).resolve().parent
RUNNER = HERE / "magicpen_nocurve_paint_capture.py"
FIXTURE = HERE / "magicpen-nocurve-paint-native.json"
CAPTURE_HELPER_SHA256 = "76c85701436f716cefae525c5a50ee68e062616509a57239aeea1e8e63135601"
PYTHON_SHA256 = "889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9"
RUNTIME_TREE_SHA256 = "49890833f43797f365f9c2e908196df31ad8bfc6dd003e3b1965f601e2a30230"
UNICORN_VERSION = "2.1.4"
UNICORN_ROOT = ROOT / "scratch/apk-analysis-runtime/python/unicorn"
RUNTIME_PYTHON = ROOT / "scratch/apk-analysis-runtime/python"
CONFORMANCE = ROOT / "conformance"
BINUTILS_LIB = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
CASES = tuple(f"{background}-{scale}x" for scale in (1, 2, 4) for background in ("opaque", "transparent"))
EXPECTED_VECTOR_SHA256 = "170dda4d45a289d32db43b8450b2221d44c4090414ee2da65283c9ce652f1522"
EXPECTED_SOURCE_CAPTURE_SHA256 = "a053123d00b14ffeffc99cfed46af8579b224febca9ceb39ea2f313662cbc018"
EXPECTED_RGBA_SHA256 = {
    "opaque-1x": "1d4f119a16dfbc2ab1608ac5d077a6a2e7a5c1e2d9c45074e69001e8702cc79b",
    "transparent-1x": "2f88aeeb45ee710f27bab8dccc7a32a5197e60e34c357b198c046711b67c6bb3",
    "opaque-2x": "5b0dc45920988e5e8145e11bae2a4a964b331a7b598bae0e178d43155d58909c",
    "transparent-2x": "23d802d46449552c0c173aa789abf5c2cfc97e9b0ded9de793263a15adbb2bce",
    "opaque-4x": "bb63343de85909598a4a6de353c7f8f00d6d2fb0589c9d6c99b67ba459885a21",
    "transparent-4x": "e85210a7ced42e5724dc5466dd16d4ff7c0bf36439430fa0beac933e366bd06b",
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_digest(path: Path) -> str:
    return digest(path.read_bytes())


def runtime_tree_digest() -> str:
    rows = []
    for path in sorted(RUNTIME_PYTHON.rglob("*")):
        if path.is_file() and "__pycache__" not in path.parts and path.suffix != ".pyc":
            rel = path.relative_to(RUNTIME_PYTHON).as_posix()
            rows.append(rel.encode() + b"\0" + file_digest(path).encode() + b"\n")
    return digest(b"".join(rows))


def runner_pin_maps() -> tuple[dict[str, str], dict[str, str], dict[str, str]]:
    tree = ast.parse(RUNNER.read_text())
    wanted = {"HELPER_HASHES", "DSO_HASHES", "TOOL_HASHES"}
    found = {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            name = node.targets[0].id
            if name in wanted:
                found[name] = ast.literal_eval(node.value)
    if set(found) != wanted:
        raise SystemExit("capture runner dependency pin maps are missing or changed")
    return found["HELPER_HASHES"], found["DSO_HASHES"], found["TOOL_HASHES"]


def check_pins() -> tuple[dict[str, str], dict[str, str], dict[str, str]]:
    if file_digest(RUNNER) != CAPTURE_HELPER_SHA256:
        raise SystemExit(f"capture helper changed: {RUNNER}")
    if Path(sys.executable).resolve() != Path("/usr/bin/python3").resolve() or file_digest(Path(sys.executable).resolve()) != PYTHON_SHA256:
        raise SystemExit("Python executable differs from the pinned /usr/bin/python3")
    if sys.version_info[:3] != (3, 13, 5):
        raise SystemExit(f"Python runtime version changed: {sys.version.split()[0]}")
    if runtime_tree_digest() != RUNTIME_TREE_SHA256:
        raise SystemExit("pinned Unicorn runtime package changed")
    metadata = (RUNTIME_PYTHON / "unicorn-2.1.4.dist-info/METADATA").read_text()
    if "\nVersion: 2.1.4\n" not in metadata:
        raise SystemExit("pinned Unicorn package version changed")
    helper_hashes, dso_hashes, tool_hashes = runner_pin_maps()
    for rel, expected in helper_hashes.items():
        path = ROOT / rel
        if file_digest(path) != expected:
            raise SystemExit(f"pinned helper changed: {rel}")
    libdir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    for name, expected in dso_hashes.items():
        if file_digest(libdir / name) != expected:
            raise SystemExit(f"pinned native DSO changed: {name}")
    for name, expected in tool_hashes.items():
        path = Path(name) if name.startswith("/") else ROOT / name
        if file_digest(path) != expected:
            raise SystemExit(f"pinned tool/runtime library changed: {name}")
    pinned_path = os.pathsep.join((str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"), "/usr/bin"))
    required_tools = {
        "nm": Path("/usr/bin/nm").resolve(),
        "c++filt": Path("/usr/bin/c++filt").resolve(),
        "aarch64-linux-gnu-objdump": (ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump").resolve(),
    }
    for name, expected_path in required_tools.items():
        actual = shutil.which(name, path=pinned_path)
        if actual is None or Path(actual).resolve() != expected_path:
            raise SystemExit(f"{name} resolves to an unexpected executable: {actual}")
    return helper_hashes, dso_hashes, tool_hashes


def validate_case(case_id: str, value: dict, source_capture: dict | None) -> dict:
    if set(value) != {"source_capture", "paint"}:
        raise SystemExit(f"{case_id}: unexpected capture top-level shape")
    capture, paint = value["source_capture"], value["paint"]
    if source_capture is not None and capture != source_capture:
        raise SystemExit(f"{case_id}: source/model Drawing capture drifted across controls")
    source = capture["source_input"]
    if source["points"] != [[0, 0], [10, 0], [20, 2]] or source["curve_enabled"] is not False:
        raise SystemExit(f"{case_id}: unexpected native source points or curve mode")
    if source["tool_type"] != 2 or source["stroke_type"] != 0 or source["color_argb"] != "0x80336699":
        raise SystemExit(f"{case_id}: unexpected source style")
    if digest(json.dumps(capture, sort_keys=True, separators=(",", ":")).encode()) != EXPECTED_SOURCE_CAPTURE_SHA256:
        raise SystemExit(f"{case_id}: source capture no longer matches the frozen source witness")
    if paint["native_vector_sha256"] != EXPECTED_VECTOR_SHA256 or paint["native_vector_bytes"] != 96:
        raise SystemExit(f"{case_id}: native queued vector stream differs")
    scale = int(case_id.rsplit("-", 1)[1][:-1])
    background = case_id.rsplit("-", 1)[0]
    expected_bg = [0, 0, 0, 0] if background == "transparent" else [64, 128, 192, 255]
    if paint["view_scale"] != scale or paint["target_initial_background"] != {"kind": background, "rgba8": expected_bg}:
        raise SystemExit(f"{case_id}: target scale or background differs")
    if paint["draw_matrix_scale"] != scale or paint["draw_matrix_y_sign"] != -1 or paint["draw_matrix_translation"] != [0, 256 * scale]:
        raise SystemExit(f"{case_id}: source-to-target transform differs")
    readback = paint["readback"]
    raw = zlib.decompress(base64.b64decode(readback["rgba8_zlib_base64"], validate=True))
    expected_size = 256 * scale
    if (readback["width"], readback["height"], readback["bytes"]) != (expected_size, expected_size, expected_size * expected_size * 4):
        raise SystemExit(f"{case_id}: readback dimensions/length differ")
    if len(raw) != readback["bytes"] or digest(raw) != readback["sha256"]:
        raise SystemExit(f"{case_id}: full RGBA readback hash/length mismatch")
    if readback["sha256"] != EXPECTED_RGBA_SHA256[case_id]:
        raise SystemExit(f"{case_id}: full RGBA payload differs from the independently reviewed capture")
    queries = paint["framebuffer_bindings_before_readpixels"]
    draw_attachment, read_attachment = queries["draw_attachment_query"], queries["read_attachment_query"]
    if queries["draw_value"] != queries["read_value"] or queries["query_gl_error"] != 0:
        raise SystemExit(f"{case_id}: read/draw framebuffer binding query failed")
    if any(a["object_type"] != 5890 or a["object_name"] != 2 or a["status"] != 36053 for a in (draw_attachment, read_attachment)):
        raise SystemExit(f"{case_id}: attachment query or framebuffer completeness differs")
    if paint["gl_errors_final"] != 0 or not paint["framebuffer_completeness"]["all_complete"]:
        raise SystemExit(f"{case_id}: native host GLES draw reported errors or incomplete target")
    return capture


def fresh_cases(helper_hashes: dict[str, str], dso_hashes: dict[str, str], tool_hashes: dict[str, str]) -> dict[str, dict]:
    env = {
        "PATH": os.pathsep.join((str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"), "/usr/bin")),
        "PYTHONPATH": os.pathsep.join((str(RUNTIME_PYTHON), str(CONFORMANCE))),
        "LD_LIBRARY_PATH": str(BINUTILS_LIB),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
    }
    results = {}
    source_capture = None
    for scale in (1, 2, 4):
        for background in ("opaque", "transparent"):
            case_id = f"{background}-{scale}x"
            env["MAGICPEN_SCALE"] = str(scale)
            proc = subprocess.run(
                [sys.executable, str(RUNNER), "--_capture-background", background],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            if proc.returncode != 0:
                raise SystemExit(f"{case_id} native replay failed ({proc.returncode}):\n{proc.stdout}\n{proc.stderr}")
            try:
                value = json.loads(proc.stdout)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"{case_id} runner did not emit JSON: {exc}\n{proc.stdout}") from exc
            source_capture = validate_case(case_id, value, source_capture)
            results[case_id] = value
    return results


def encoded_fixture(cases: dict[str, dict]) -> bytes:
    fixture = {
        "schema": "magicpen-nocurve-native-paint-six-v1",
        "wrapper_sha256": file_digest(Path(__file__).resolve()),
        "capture_helper_sha256": CAPTURE_HELPER_SHA256,
        "python_sha256": PYTHON_SHA256,
        "unicorn_runtime_sha256": RUNTIME_TREE_SHA256,
        "case_order": list(CASES),
        "cases": cases,
        "scope": "Synthetic Model/ObjectStroke source and host resource/callback adapters; native Drawing selection, MagicPen GLV1, queued vector upload and Renderer Draw execute. RGBA target and initial background are host-owned; rendering uses the pinned host Mesa EGL/GLES stack, not an Android device. Opaque RGB64/128/192 cases are blend controls; transparent cases are the native compositing comparison inputs.",
    }
    return (json.dumps(fixture, indent=2, sort_keys=True) + "\n").encode()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true", help="freshly capture all six cases and explicitly replace the fixture")
    modes.add_argument("--check-fixture", action="store_true", help="freshly replay all six cases and compare with the fixture (default)")
    args = parser.parse_args()
    wrapper_sha256 = file_digest(Path(__file__).resolve())
    if not args.write_fixture:
        if not FIXTURE.is_file():
            raise SystemExit(f"missing frozen fixture: {FIXTURE}")
        frozen = json.loads(FIXTURE.read_text())
        if frozen.get("wrapper_sha256") != wrapper_sha256:
            raise SystemExit("wrapper SHA-256 differs from the frozen fixture; explicit --write-fixture is required after review")
        if frozen.get("capture_helper_sha256") != CAPTURE_HELPER_SHA256:
            raise SystemExit("capture helper SHA-256 differs from the frozen fixture")
    helper_hashes, dso_hashes, tool_hashes = check_pins()
    cases = fresh_cases(helper_hashes, dso_hashes, tool_hashes)
    raw = encoded_fixture(cases)
    if args.write_fixture:
        fd, temp_name = tempfile.mkstemp(prefix=FIXTURE.name + ".", dir=FIXTURE.parent)
        try:
            with os.fdopen(fd, "wb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temp_name, FIXTURE)
        finally:
            if os.path.exists(temp_name):
                os.unlink(temp_name)
        print(f"wrote {FIXTURE.relative_to(ROOT)} sha256={digest(raw)}")
        return 0
    expected = FIXTURE.read_bytes()
    if expected != raw:
        raise SystemExit(f"fresh six-case capture differs from {FIXTURE.relative_to(ROOT)}; use --write-fixture explicitly")
    print(f"verified {FIXTURE.relative_to(ROOT)} sha256={digest(raw)} cases={len(cases)} wrapper_sha256={wrapper_sha256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
