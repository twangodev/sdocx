"""Conformance oracle for Samsung's native PDFium-backed StandardPDF exporter."""
import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-paper-pdfium-standard-export.json"
DRIVER = Path("/tmp/sdocx-pdf-paper-standard-export")
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"
LIBCPP_SHA256 = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"
SPENPDF_SHA256 = "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04"
SPENBASE_SHA256 = "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
LIBM_PATH = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")
LIBM_SHA256 = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"
sys.path.insert(0, str(ROOT / "conformance"))
from pdf_paper_pdfium_source_overlay_native import source_output, source_pdf


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture() -> dict:
    for path, expected, label in [
        (APK_DIR / "libSPenModel.so", MODEL_SHA256, "model"),
        (APK_DIR / "libSPenPdfiumB.so", PDFIUM_SHA256, "PDFium"),
        (APK_DIR / "libc++_shared.so", LIBCPP_SHA256, "libc++"),
        (APK_DIR / "libSPenPdf.so", SPENPDF_SHA256, "Samsung PDF"),
        (APK_DIR / "libSPenBase.so", SPENBASE_SHA256, "Samsung base"),
        (LIBM_PATH, LIBM_SHA256, "host libm"),
    ]:
        actual = sha(path)
        if actual != expected:
            raise SystemExit(f"{label} SHA-256 changed: {actual}")

    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN_LIB}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)

    cases = []
    for rotation in (0, 90, 180, 270):
        source_bytes = source_pdf(rotation)
        source_path = Path(f"/tmp/sdocx-pdf-paper-standard-source-{rotation}.pdf")
        output_path = Path(f"/tmp/sdocx-pdf-paper-standard-export-{rotation}.pdf")
        source_path.write_bytes(source_bytes)
        output_path.unlink(missing_ok=True)
        run = subprocess.run([
            str(DRIVER), str(APK_DIR / "libSPenModel.so"),
            "--pdfium-standard-export", str(APK_DIR / "libSPenPdfiumB.so"),
            str(source_path), str(output_path),
        ], cwd=ROOT, text=True, capture_output=True, check=True)
        native = json.loads(run.stdout)
        if "PdfiumExportHandler::Export(inclusive page range 0..0)" not in native["native_path"]:
            raise SystemExit(f"rotation {rotation}: native StandardPDF export path was not executed")
        pdf = output_path.read_bytes()
        case = source_output(pdf, rotation, hashlib.sha256(source_bytes).hexdigest(), output_path, native)
        case["output_file_sha256"] = sha(output_path)
        cases.append(case)

    return {
        "source_apk": {"version": "4.4.45.37", "sha256": "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"},
        "model_library_sha256": MODEL_SHA256,
        "pdfium_library_sha256": PDFIUM_SHA256,
        "libcxx_library_sha256": LIBCPP_SHA256,
        "samsung_pdf_library_sha256": SPENPDF_SHA256,
        "samsung_base_library_sha256": SPENBASE_SHA256,
        "host_libm_sha256": LIBM_SHA256,
        "cases": cases,
        "evidence_boundary": (
            "Executes the pinned Samsung PdfiumImpl constructor, file-backed Open, LoadPage, "
            "PdfiumExportHandler OpenWriter/CloseWriter/Export inclusive range 0..0, actual "
            "PdfiumPathHandler DrawPath, and PDFFileWriter through PDFium serialization. The "
            "source PDF and overlay points are deterministic synthetic host inputs. Output checks "
            "prove the original vector path and tested boxes, rotation and UserUnit survive this "
            "one-page fixture while the Samsung vector overlay is serialized in the retained page. "
            "The host supplies file I/O and selected deterministic C/libm/platform callbacks; "
            "single-thread synchronization is a no-op and unknown imports abort. This does not "
            "prove arbitrary-PDF preservation, Android rendering parity, or execution of the full app/JNI route."
        ),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    encoded = json.dumps(capture(), indent=2) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("Samsung StandardPDF export differs from conformance/pdf-paper-pdfium-standard-export.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
