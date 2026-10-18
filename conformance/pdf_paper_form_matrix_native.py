"""Replay the bounded PDFium form-matrix fragment used by Standard PDF export.

Run from the repository root with the pinned APK extraction and bundled Unicorn:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_form_matrix_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_form_matrix_native.py --check-fixture
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-paper-form-matrix.json"
DRIVER = Path("/tmp/sdocx-pdf-paper-form-matrix")
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"


def capture():
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", "native=scratch/apk-analysis-runtime/python/unicorn/lib",
        "-C", "link-arg=-Wl,-rpath," + str(ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"),
        "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    output = subprocess.check_output([
        str(DRIVER), str(APK_DIR / "libSPenModel.so"), "--pdf-paper-form-matrix",
        str(APK_DIR / "libSPenPdfiumB.so"),
    ], cwd=ROOT, text=True)
    result = json.loads(output)
    if result["pdfium_sha256"] != PDFIUM_SHA256:
        raise SystemExit("loaded PDFium hash differs from the frozen form-matrix provenance")
    result["source_apk"] = {
        "version": "4.4.45.37",
        "sha256": "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667",
    }
    result["model_library_sha256"] = MODEL_SHA256
    result["case_boundary"] = (
        "The runner executes pinned PDFium CPDF_Page::UpdateDimensions and GetPageWidth/GetPageHeight "
        "with host-supplied page dictionary values, then original FPDFPage_NewFormObj instructions "
        "from +0x50 to +0x104. Page-handle dispatch, "
        "document/XObject construction, complete Samsung writer, PDF serialization and rendering "
        "are excluded. Each page_rotation_quarter_turn is distinct from the OpenWriter form argument "
        "4-page_rotation; PDFium normalizes the latter modulo 4 to select the captured matrix. "
        "Candidate top-left point outputs compose the matrix with a host-supplied point and host-side "
        "H-y arithmetic using the actual native GetPageHeight result after UpdateDimensions has "
        "applied source rotation. Source page dictionary values and point remain host-supplied; the "
        "point/matrix composition is host-side, not a native full-write capture."
    )
    return json.dumps(result, indent=2, separators=(",", ": ")) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    encoded = capture()
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native PDF-paper form-matrix output differs from conformance/pdf-paper-form-matrix.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
