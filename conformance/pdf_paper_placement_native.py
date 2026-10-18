"""Replay the bounded Samsung Notes PDF paper placement fragment.

Run from the repository root with the pinned APK extraction and Unicorn:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_placement_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_placement_native.py --check-fixture
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-paper-placement.json"
DRIVER = Path("/tmp/sdocx-native-pdf-paper-placement")


def capture(args):
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", "native=scratch/apk-analysis-runtime/python/unicorn/lib",
        "-C", "link-arg=-Wl,-rpath," + str(ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"),
        "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    model = args.model or APK_DIR / "libSPenModel.so"
    composer = args.composer or APK_DIR / "libSPenComposer.so"
    base = args.base or APK_DIR / "libSPenBase.so"
    view = args.view or APK_DIR / "libSPenView.so"
    return subprocess.check_output([
        str(DRIVER), str(model), "--pdf-paper-placement", str(composer), str(base), str(view)
    ], cwd=ROOT, text=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--model", type=Path)
    parser.add_argument("--composer", type=Path)
    parser.add_argument("--base", type=Path)
    parser.add_argument("--view", type=Path)
    args = parser.parse_args()
    encoded = json.dumps(json.loads(capture(args)), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native PDF-paper placement output differs from conformance/pdf-paper-placement.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
