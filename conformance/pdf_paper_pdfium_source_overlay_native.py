"""Run pinned PDFium's native source-PDF load and vector-overlay sequence.

Run from the repository root with the extracted Samsung Notes APK and bundled
Unicorn runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_pdfium_source_overlay_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_paper_pdfium_source_overlay_native.py --check-fixture
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
FIXTURE = ROOT / "conformance/pdf-paper-pdfium-source-overlay.json"
DRIVER = Path("/tmp/sdocx-pdfium-source-overlay-conformance")
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"
LIBCPP_SHA256 = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"
LIBM_PATH = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")
LIBM_SHA256 = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"


def source_pdf(rotation: int) -> bytes:
    """Build a tiny deterministic classic-xref PDF using only the stdlib."""
    if rotation not in (0, 90, 180, 270):
        raise ValueError(rotation)
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        (
            f"<< /Type /Page /Parent 2 0 R /MediaBox [-20 -10 180 140] "
            f"/CropBox [0 5 150 120] /Rotate {rotation} /UserUnit 2 "
            "/Resources << >> /Contents 4 0 R >>"
        ).encode("ascii"),
    ]
    content = b"17 21 m 91 44 l 57 118 l h 0 0 1 rg f\n"
    objects.append(b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"endstream")
    pdf = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for number, body in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf += f"{number} 0 obj\n".encode("ascii") + body + b"\nendobj\n"
    xref = len(pdf)
    pdf += f"xref\n0 {len(offsets)}\n".encode("ascii")
    pdf += b"0000000000 65535 f \n"
    for offset in offsets[1:]:
        pdf += f"{offset:010} 00000 n \n".encode("ascii")
    pdf += (
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n"
        f"startxref\n{xref}\n%%EOF\n"
    ).encode("ascii")
    return bytes(pdf)


def object_body(pdf: bytes, number: int) -> bytes:
    match = re.search(rb"(?m)^" + str(number).encode() + rb" 0 obj\s*(.*?)\s*endobj", pdf, re.S)
    if not match:
        raise SystemExit(f"missing PDF object {number}")
    return match.group(1)


def source_output(pdf: bytes, rotation: int, source_hash: str, output_path: Path, native: dict) -> dict:
    page_number = None
    page_body = None
    for match in re.finditer(rb"(?m)^(\d+) 0 obj\s*(.*?)\s*endobj", pdf, re.S):
        body = match.group(2)
        if re.search(rb"/Type\s*/Page(?:\s|/)", body) and not re.search(rb"/Type\s*/Pages\b", body):
            page_number = int(match.group(1))
            page_body = body
            break
    if page_number is None:
        raise SystemExit("serialized source page dictionary missing")
    content_match = re.search(rb"/Contents\s*(\[[^]]+\]|\d+\s+\d+\s+R)", page_body)
    if not content_match:
        raise SystemExit("serialized source content reference missing")
    direct_ref = re.match(rb"(\d+)\s+\d+\s+R", content_match.group(1))
    if direct_ref:
        content_object_number = int(direct_ref.group(1))
        content_object = object_body(pdf, content_object_number)
        content_refs = re.findall(rb"(\d+)\s+\d+\s+R", content_object) if content_object.lstrip().startswith(b"[") else [direct_ref.group(1)]
    else:
        content_refs = re.findall(rb"(\d+)\s+\d+\s+R", content_match.group(1))
    page_stream_parts = []
    for content_number in content_refs:
        body = object_body(pdf, int(content_number))
        stream = re.search(rb"stream\r?\n(.*?)\r?\nendstream", body, re.S)
        if not stream:
            raise SystemExit("serialized page content stream missing")
        data = stream.group(1)
        page_stream_parts.append(zlib.decompress(data) if b"/FlateDecode" in body else data)
    page_stream = b"\n".join(page_stream_parts)
    form_ref = re.search(rb"/FXX1\s+(\d+)\s+0\s+R", page_body)
    if not form_ref:
        raise SystemExit("native page has no overlay XObject resource")
    form_number = int(form_ref.group(1))
    form_body = object_body(pdf, form_number)
    form_stream_match = re.search(rb"stream\r?\n(.*?)\r?\nendstream", form_body, re.S)
    if not form_stream_match:
        raise SystemExit("serialized overlay form stream missing")
    form_stream = form_stream_match.group(1)
    if b"/FlateDecode" in form_body:
        form_stream = zlib.decompress(form_stream)
    matrix_match = re.search(
        rb"([\d.-]+\s+[\d.-]+\s+[\d.-]+\s+[\d.-]+\s+[\d.-]+\s+[\d.-]+)\s+cm\s+/FXX1\s+Do",
        page_stream,
    )
    if not matrix_match:
        raise SystemExit("native page form placement matrix missing")
    matrix = [float(value) for value in matrix_match.group(1).split()]
    expected_matrix = {
        0: [1, 0, 0, 1, 0, 5],
        90: [0, 1, -1, 0, 150, 5],
        180: [-1, 0, 0, -1, 150, 120],
        270: [0, -1, 1, 0, 0, 120],
    }[rotation]
    if matrix != expected_matrix:
        raise SystemExit(f"source rotation {rotation}: form matrix {matrix} != {expected_matrix}")
    if b"/MediaBox[ -20 -10 180 140]" not in page_body:
        raise SystemExit("source MediaBox changed")
    if b"/CropBox[ 0 5 150 120]" not in page_body:
        raise SystemExit("source CropBox changed")
    if f"/Rotate {rotation}".encode() not in page_body:
        raise SystemExit("source Rotate changed")
    if b"/UserUnit 2" not in page_body:
        raise SystemExit("source UserUnit changed")
    dims = [150, 115] if rotation in (0, 180) else [115, 150]
    source_path = b"17 21 m 91 44 l 57 118 l h 0 0 1 rg f"
    if source_path not in page_stream:
        raise SystemExit("original source vector content was not retained")
    standard_export = "export_result" in native
    if standard_export:
        height = dims[1]
        overlay_path = (
            f"17.25 {height - 29.5:g} m 93.75 {height - 46.5:g} l "
            f"61.25 {height - 109.75:g} l h f*"
        ).encode("ascii")
    else:
        overlay_path = b"17.25 29.5 m 93.75 46.5 l 61.25 109.75 l h B*"
    if overlay_path not in form_stream:
        raise SystemExit("native vector overlay path is missing")
    bbox_match = re.search(rb"/BBox\s*\[\s*([^\]]+)\]", form_body)
    if not bbox_match:
        raise SystemExit("native overlay form BBox missing")
    form_bbox = [float(value) for value in bbox_match.group(1).split()]
    if form_bbox != [0, 0, *dims]:
        raise SystemExit(f"native overlay form BBox changed: {form_bbox}")
    if native["loaded_width"] != dims[0] or native["loaded_height"] != dims[1]:
        raise SystemExit(f"native loaded dimensions disagree for rotation {rotation}")
    if native["source_rotation"] != rotation // 90:
        raise SystemExit(f"native source rotation changed for {rotation}")
    standard_export = "export_result" in native
    if standard_export:
        if native["load_result"] != 1 or native["writer_open"] != 1 or native["writer_close"] != 1 or native["export_result"] != 0:
            raise SystemExit(f"Samsung StandardPDF export path failed for rotation {rotation}")
        if native["output_bytes"] != len(pdf):
            raise SystemExit(f"Samsung PDFFileWriter output length changed for {rotation}")
    else:
        if native["writer_form_argument_4_minus_rotation"] != 4 - rotation // 90:
            raise SystemExit(f"native form rotation argument changed for {rotation}")
        if native["rotation_after"] != rotation // 90 or native["save_result"] != 1:
            raise SystemExit(f"native page rotation/save result changed for {rotation}")
        if native["write_calls"] != 1 or native["pdf_bytes"] != len(pdf):
            raise SystemExit(f"native writer callback output changed for {rotation}")
    return {
        "source_rotation_degrees": rotation,
        "input_pdf_sha256": source_hash,
        "serialized_pdf_sha256": hashlib.sha256(pdf).hexdigest(),
        "serialized_pdf_bytes": len(pdf),
        "pdfium_native": {
            "loaded_width": native["loaded_width"],
            "loaded_height": native["loaded_height"],
            "source_rotation_quarter_turns": native["source_rotation"],
            **({
                "samsung_load_result": native["load_result"],
                "open_writer_result": native["writer_open"],
                "close_writer_result": native["writer_close"],
                "standard_export_result": native["export_result"],
            } if standard_export else {
                "new_form_argument_quarter_turns": native["writer_form_argument_4_minus_rotation"],
                "rotation_after": native["rotation_after"],
                "form_object_count": native["form_object_count"],
                "page_object_count": native["page_object_count"],
                "page_generate_content_result": native["page_generate_content_result"],
                "save_result": native["save_result"],
                "write_calls": native["write_calls"],
            }),
            "host_imports": native["handled_imports"],
            "host_platform_events": native["platform_events"],
        },
        "retained_page": {
            "media_box": [-20, -10, 180, 140],
            "crop_box": [0, 5, 150, 120],
            "rotate_degrees": rotation,
            "user_unit": 2,
            "original_vector_content": "17 21 m 91 44 l 57 118 l h 0 0 1 rg f",
            "overlay_form_bbox": form_bbox,
            "page_form_matrix": matrix,
            "overlay_vector_points": [[17.25, 29.5], [93.75, 46.5], [61.25, 109.75]],
            "overlay_vector_content": "17.25 29.5 m 93.75 46.5 l 61.25 109.75 l h B*",
        },
    }


def capture() -> dict:
    libm_hash = hashlib.sha256(LIBM_PATH.read_bytes()).hexdigest()
    if libm_hash != LIBM_SHA256:
        raise SystemExit(f"host libm SHA-256 changed: {libm_hash}")
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN_LIB}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    cases = []
    for rotation in (0, 90, 180, 270):
        source = source_pdf(rotation)
        source_path = Path(f"/tmp/sdocx-pdf-paper-source-{rotation}.pdf")
        output_path = Path(f"/tmp/sdocx-pdfium-source-overlay-{rotation}.pdf")
        source_path.write_bytes(source)
        run = subprocess.run([
            str(DRIVER), str(APK_DIR / "libSPenModel.so"),
            "--pdfium-load-source-overlay", str(APK_DIR / "libSPenPdfiumB.so"),
            str(source_path), str(output_path),
        ], cwd=ROOT, text=True, capture_output=True, check=True)
        native = json.loads(run.stdout)
        pdf = output_path.read_bytes()
        cases.append(source_output(pdf, rotation, hashlib.sha256(source).hexdigest(), output_path, native))
    return {
        "source_apk": {"version": "4.4.45.37", "sha256": "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"},
        "model_library_sha256": MODEL_SHA256,
        "pdfium_library_sha256": PDFIUM_SHA256,
        "libcxx_library_sha256": LIBCPP_SHA256,
        "host_libm_sha256": libm_hash,
        "cases": cases,
        "evidence_boundary": (
            "Executes pinned PDFium's real FPDF_LoadMemDocument parser on deterministic synthetic PDFs, "
            "then FPDF_LoadPage, width/height/rotation queries, NewFormObj with 4-minus-page-turn, "
            "native vector form generation, and FPDF_SaveAsCopy/FILEWRITE. Input boxes, source path, "
            "overlay points, and content are synthetic host data. The source MediaBox, CropBox, Rotate, "
            "UserUnit, and source vector content are checked in exact serialized output. This does not "
            "execute SPen::PdfiumExportHandler::OpenWriter or PdfiumImpl::LoadPage, exercise a full "
            "Samsung StandardPDF export, prove arbitrary source-PDF preservation, or establish rendered "
            "pixel parity. Android platform services are explicitly bounded host providers and unknown "
            "imports abort the probe."
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
        raise SystemExit("native PDFium source-overlay output differs from conformance/pdf-paper-pdfium-source-overlay.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
