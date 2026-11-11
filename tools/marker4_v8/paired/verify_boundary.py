"""Verify the historical Marker4 size boundary against its retained receipt.

Native prerequisites remain external. This entrypoint writes captures only in a
temporary directory; boundary_capture.py and its historical bytes stay intact.
"""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[3]
SIZES = (0.99, 1.0, 1.5, 1.99)
HISTORICAL_PREFIX = "/home/jding/RustroverProjects/sdocx/scratch/defaultpen-native/fresh-marker4-"
PINS = {
    "scratch/defaultpen-native/marker4_native_capture_full_tool2.py":
        "187ccdb582218ea898d1fbe7b536e25bd8241bc2518ed7efc6110ba84cb9851f",
    "scratch/defaultpen-native/native-marker4-v8-small.json":
        "c8b2ba57b36ea7d548082c9310c07f58b8bff3143d0e981314af66bd0f8a40a8",
    "tmp/vector-parity-plan/chinesebrush-paint-scale-work/scratch/apk-analysis-native/arm64-v8a/libSPenMarker4.so":
        "23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff",
}


def load_boundary_receipt(root):
    retained = json.loads((root / "conformance/marker4-v8-thin-host-native.json").read_text())
    return json.loads(retained["paired_receipts_original"]["boundary"])


def read_pinned_inputs(root):
    sources = {}
    for relative, expected in PINS.items():
        data = (root / relative).read_bytes()
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"Historical input hash mismatch: {relative}")
        sources[relative] = data
    fixture = json.loads(sources["scratch/defaultpen-native/native-marker4-v8-small.json"])
    inputs = json.loads((root / "tools/marker4_v8/paired/inputs.json").read_bytes())
    if next(row for row in fixture if row["stroke"] == 35) != inputs["native"]:
        raise ValueError("Retained stroke 35 differs from paired inputs")
    return sources["scratch/defaultpen-native/marker4_native_capture_full_tool2.py"].decode()


def prepare_size_runner(source, size, directory):
    for marker, count in (("[36.21])", 1), ("'set_pen_size_f32':36.21", 1),
                          (HISTORICAL_PREFIX, 4)):
        if source.count(marker) != count:
            raise ValueError(f"Unexpected capture source marker count: {marker}")
    return (source.replace("[36.21])", f"[{size}])")
            .replace("'set_pen_size_f32':36.21", f"'set_pen_size_f32':{size}")
            .replace(HISTORICAL_PREFIX, str(directory / "capture-")))


def verify_pixels(pixels, expected):
    actual = {
        "rgba_sha256": hashlib.sha256(pixels).hexdigest(),
        "nonzero_alpha_pixels": sum(alpha != 0 for alpha in pixels[3::4]),
        "max_alpha": max(pixels[3::4]),
    }
    for field, value in actual.items():
        if value != expected[field]:
            raise ValueError(f"Size {expected['size']}: {field}: {value} != {expected[field]}")
    return actual


def capture_size(root, source, size):
    with tempfile.TemporaryDirectory(prefix="marker4-v8-pair-") as temporary:
        directory = Path(temporary)
        runner = directory / "runner.py"
        runner.write_text(prepare_size_runner(source, size, directory))
        subprocess.run([sys.executable, str(runner), "--capture-case", "full-real",
                        "--capture-output", str(directory / "output.json")],
                       cwd=root, check=True, stdout=subprocess.DEVNULL)
        return (directory / "capture-stroke35.rgba").read_bytes()


def main():
    receipt = load_boundary_receipt(ROOT)
    expected = {case["size"]: case for case in receipt["cases"]}
    source = read_pinned_inputs(ROOT)
    for size in SIZES:
        actual = verify_pixels(capture_size(ROOT, source, size), expected[size])
        print(json.dumps({"size": size, **actual}))


if __name__ == "__main__":
    main()
