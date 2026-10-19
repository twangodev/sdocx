"""Replay the bounded Composer PDF-paper import/skip and manual extent controls.

The selected quiz PDF must be available locally (its bytes are never copied into
this repository):
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_composer_export_native.py --source-pdf /path/to/quiz.pdf --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_composer_export_native.py --source-pdf /path/to/quiz.pdf --check-fixture
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK = ROOT / "scratch/apk-analysis-native/arm64-v8a"
UNICORN = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
DRIVER = Path("/tmp/sdocx-composer-pdf-paper-export-conformance")
FIXTURE = ROOT / "conformance/pdf-paper-composer-export.json"
SOURCE_SHA256 = "f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279"


def capture(source: Path) -> dict:
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    if digest != SOURCE_SHA256:
        raise SystemExit(f"quiz source PDF SHA-256 mismatch: {digest}")
    subprocess.run([
        "rustc", "--edition=2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    raw = subprocess.check_output([
        str(DRIVER), str(APK / "libSPenModel.so"), "--composer-pdf-paper-export",
        str(APK / "libSPenComposer.so"), str(APK / "libSPenBase.so"), str(source),
    ], cwd=ROOT, text=True)
    result = json.loads(raw)
    if result["source_pdf_provenance"].split("SHA-256 ", 1)[1] != digest:
        raise SystemExit("native capture provenance does not match checked source PDF")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--source-pdf", required=True, type=Path)
    args = parser.parse_args()
    encoded = json.dumps(capture(args.source_pdf), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native Composer PDF-paper export output differs from fixture")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
