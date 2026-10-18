"""Capture saved Arc segment conversion through Drawing and native Skia.

Run from the repository root with the APK extraction and Unicorn runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/shape_drawing_arcs_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/shape_drawing_arcs_native.py --check-fixture
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/shape-drawing-arcs.json"
DRIVER = Path("/tmp/sdocx-native-table")


def capture(args):
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", "native=scratch/apk-analysis-runtime/python/unicorn/lib",
        "-C", "link-arg=-Wl,-rpath," + str(ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"),
        "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    model = args.model or APK_DIR / "libSPenModel.so"
    drawing = args.drawing or APK_DIR / "libSPenDrawing.so"
    base = args.base or APK_DIR / "libSPenBase.so"
    skia = args.skia or APK_DIR / "libSPenSkia.so"
    return subprocess.check_output([
        str(DRIVER), str(model), "--shape-drawing-arcs", str(drawing), str(base), str(skia)
    ], cwd=ROOT, text=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    group = p.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    p.add_argument("--model", type=Path)
    p.add_argument("--drawing", type=Path)
    p.add_argument("--base", type=Path)
    p.add_argument("--skia", type=Path)
    args = p.parse_args()
    actual = json.loads(capture(args))
    encoded = json.dumps(actual, indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native Drawing Arc capture differs from conformance/shape-drawing-arcs.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
