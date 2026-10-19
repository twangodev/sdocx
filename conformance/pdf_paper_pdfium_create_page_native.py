"""Replay PDFium's pinned native PDF create/form/path/save sequence.

Run from the repository root with the extracted Samsung Notes APK and bundled
Unicorn runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_pdfium_create_page_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_pdfium_create_page_native.py --check-fixture
"""
import argparse
import hashlib
import json
import re
import subprocess
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-paper-pdfium-create-page.json"
DRIVER = Path("/tmp/sdocx-pdf-paper-pdfium-create-page")
PDF = Path("/tmp/sdocx-pdfium-created-page.pdf")
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"
LIBCPP_SHA256 = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"
LIBM_PATH = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")
LIBM_SHA256 = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"
APK_SHA256 = "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
PDF_SHA256 = "70b75e9ff2c31dedbeb2320cb4300c62b4d73f8740ee4f8afd97e27e0b2ff083"


def object_body(pdf: bytes, number: int) -> bytes:
    match = re.search(rb"(?m)^" + str(number).encode() + rb" 0 obj\s*(.*?)\s*endobj", pdf, re.S)
    if not match:
        raise SystemExit(f"missing PDF object {number}")
    return match.group(1)


def stream_bytes(body: bytes) -> bytes:
    match = re.search(rb"stream\r?\n(.*?)\r?\nendstream", body, re.S)
    if not match:
        raise SystemExit("native PDF stream missing")
    data = match.group(1)
    return zlib.decompress(data) if b"/FlateDecode" in body else data


def capture() -> dict:
    libm_hash = hashlib.sha256(LIBM_PATH.read_bytes()).hexdigest()
    if libm_hash != LIBM_SHA256:
        raise SystemExit(f"host libm SHA-256 changed: {libm_hash}")
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN_LIB}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    run = subprocess.run([
        str(DRIVER), str(APK_DIR / "libSPenModel.so"), "--pdfium-create-page-save",
        str(APK_DIR / "libSPenPdfiumB.so"),
    ], cwd=ROOT, text=True, capture_output=True, check=True)
    native = json.loads(run.stdout)
    pdf = PDF.read_bytes()
    pdf_hash = hashlib.sha256(pdf).hexdigest()
    if pdf_hash != PDF_SHA256:
        raise SystemExit(f"serialized PDF SHA-256 changed: {pdf_hash}")
    if native["pdf_bytes"] != len(pdf) or native["write_calls"] != 1:
        raise SystemExit("native FPDF_FILEWRITE output length/callback count changed")

    page_body = object_body(pdf, 4)
    form_body = object_body(pdf, 5)
    page_content = stream_bytes(object_body(pdf, 9))
    form_content = stream_bytes(form_body)
    expected_page = b"/Rotate 90"
    expected_form = b"/BBox[ 0 0 150 200]"
    if expected_page not in page_body or b"/MediaBox[ 0 0 200 150]" not in page_body:
        raise SystemExit("native page media box or rotation changed")
    if expected_form not in form_body:
        raise SystemExit("native form BBox changed")
    if b"/FXE1 8 0 R" not in page_body or b"/FXX1 5 0 R" not in page_body:
        raise SystemExit("native page resource mapping changed")
    if b"0 1 -1 0 200 0 cm /FXX1 Do" not in page_content:
        raise SystemExit("native form placement matrix/resource use changed")
    expected_path = b"21.5 33.25 m 91.5 44.25 l 56.75 128.5 l h B*"
    if expected_path not in form_content or b"/FXE1 gs" not in form_content:
        raise SystemExit("native form vector path or graphics-state resource changed")

    return {
        "source_apk": {"version": "4.4.45.37", "sha256": APK_SHA256},
        "model_library_sha256": MODEL_SHA256,
        "pdfium_library_sha256": PDFIUM_SHA256,
        "libcxx_library_sha256": LIBCPP_SHA256,
        "host_libm_sha256": libm_hash,
        "native_capture": {
            "native_path": native["native_path"],
            "source_rotation": native["source_rotation"],
            "writer_form_argument_4_minus_rotation": native["writer_form_argument_4_minus_rotation"],
            "rotation_after": native["rotation_after"],
            "form_object_count": native["form_object_count"],
            "page_object_count": native["page_object_count"],
            "page_generate_content_result": native["page_generate_content_result"],
            "save_result": native["save_result"],
            "pdf_bytes": native["pdf_bytes"],
            "write_calls": native["write_calls"],
            "handled_imports": native["handled_imports"],
            "platform_events": native["platform_events"],
        },
        "serialized_pdf_sha256": pdf_hash,
        "pdf_geometry": {
            "page_media_box": [0, 0, 200, 150],
            "page_rotate": 90,
            "form_bbox": [0, 0, 150, 200],
            "page_form_matrix": [0, 1, -1, 0, 200, 0],
            "form_path_points": [[21.5, 33.25], [91.5, 44.25], [56.75, 128.5]],
            "resources": {"extgstate": "FXE1", "xobject": "FXX1"},
        },
        "evidence_boundary": (
            "Executes pinned PDFium document/page/form/path/resource generation and PDF save, "
            "then parses the exact serialized PDF bytes. Host-supplied blank page dimensions, "
            "rotation, triangle points, color, and FILEWRITE callback are synthetic. It does not "
            "load a source PDF, exercise Samsung SPenPdfOpenWriter, prove source CropBox/Rotate/" 
            "UserUnit placement, or establish Samsung/native pixel parity. Host hypotf uses "
            "hash-pinned Linux libm, not Android libm. The checked formatter handles only observed "
            "conversions. C-locale multibyte conversion and epoch-zero UTC time are deterministic "
            "host services; directory enumeration is represented as unavailable. Mutex/condition "
            "calls are single-thread no-ops, which provides no concurrency evidence. Host allocation "
            "and zlib are explicit providers. Every imported host symbol is recorded; an unhandled "
            "import aborts the replay."
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
        raise SystemExit("native PDFium create-page output differs from conformance/pdf-paper-pdfium-create-page.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
