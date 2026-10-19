"""Replay the bounded native Composer SingleX segmentation witness.

Requires the retained quiz note used to identify the source record:
  python3 conformance/pdf_paper_single_segments_native.py --source-note tmp/stroke-conformance/quiz.sdocx --write-fixture
  python3 conformance/pdf_paper_single_segments_native.py --source-note tmp/stroke-conformance/quiz.sdocx --check-fixture

The note bytes are validated by SHA-256 and are not copied into this repository.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK = ROOT / "scratch/apk-analysis-native/arm64-v8a"
UNICORN = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
DRIVER = Path("/tmp/sdocx-pdf-paper-single-segments-conformance")
FIXTURE = ROOT / "conformance/pdf-paper-single-segments.json"
NOTE_SHA256 = "38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a"


def capture(source_note: Path) -> dict:
    digest = hashlib.sha256(source_note.read_bytes()).hexdigest()
    if digest != NOTE_SHA256:
        raise SystemExit(f"retained quiz note SHA-256 mismatch: {digest}")
    subprocess.run([
        "rustc", "--edition=2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    raw = subprocess.check_output([
        str(DRIVER), str(APK / "libSPenModel.so"), "--pdf-paper-single-segments",
        str(APK / "libSPenComposer.so"), str(APK / "libSPenBase.so"),
    ], cwd=ROOT, text=True)
    result = json.loads(raw)
    if result["source_note_sha256"] != digest:
        raise SystemExit("native capture note provenance differs from checked source note")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--source-note", required=True, type=Path)
    args = parser.parse_args()
    encoded = json.dumps(capture(args.source_note), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native SingleX segmentation output differs from fixture")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
