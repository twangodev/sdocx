"""Capture direct Model unit-arc builder output before the outer 0x211814 mapping.

Run from the repository root with the APK extraction and Unicorn runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/model_quad_arcs_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/model_quad_arcs_native.py --check-fixture
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/model-quad-arcs.json"
DRIVER = Path("/tmp/sdocx-native-model-quad-arcs")


def capture(args):
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", "native=scratch/apk-analysis-runtime/python/unicorn/lib",
        "-C", "link-arg=-Wl,-rpath," + str(ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"),
        "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    model = args.model or APK_DIR / "libSPenModel.so"
    base = args.base or APK_DIR / "libSPenBase.so"
    skia = args.skia or APK_DIR / "libSPenSkia.so"
    return subprocess.check_output([
        str(DRIVER), str(model), "--model-quad-arcs", str(base), str(skia)
    ], cwd=ROOT, text=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--model", type=Path)
    parser.add_argument("--base", type=Path)
    parser.add_argument("--skia", type=Path)
    args = parser.parse_args()
    encoded = json.dumps(json.loads(capture(args)), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("direct native Model unit-arc output differs from conformance/model-quad-arcs.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
