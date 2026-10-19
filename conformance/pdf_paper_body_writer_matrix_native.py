"""Capture Composer BodyText transformation followed by separate PDFium placement helpers.

Run from the repository root with the pinned APK extraction and bundled Unicorn:
  python3 conformance/pdf_paper_body_writer_matrix_native.py --write-fixture
  python3 conformance/pdf_paper_body_writer_matrix_native.py --check-fixture

The native Composer DrawObject sink and following PDFium matrix helper are separate
calls. This fixture does not claim the full application insertion path.
"""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
FIXTURE = ROOT / "conformance/pdf-paper-body-writer-matrix.json"
DRIVER = Path("/tmp/sdocx-pdf-paper-body-writer-matrix")

LIBRARY_HASHES = {
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenPdf.so": "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04",
    "libSPenPdfiumB.so": "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}

NOTE_PAGE = [1812, 15372]
MARKER = {"rect": [10, 20, 40, 60], "position": [23, 17]}
MEDIA_BOX = [-20, -10, 180, 140]
USER_UNIT = 2


def source_pdf(rotation: int, crop_box: list[int]) -> bytes:
    if rotation not in (90, 270):
        raise ValueError(rotation)
    if len(crop_box) != 4 or crop_box[2] - crop_box[0] != 150 or crop_box[3] - crop_box[1] != 115:
        raise ValueError(crop_box)
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        (
            f"<< /Type /Page /Parent 2 0 R /MediaBox [-20 -10 180 140] "
            f"/CropBox [{crop_box[0]} {crop_box[1]} {crop_box[2]} {crop_box[3]}] "
            f"/Rotate {rotation} /UserUnit 2 /Resources << >> /Contents 4 0 R >>"
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
    pdf += f"xref\n0 {len(offsets)}\n".encode("ascii") + b"0000000000 65535 f \n"
    for offset in offsets[1:]:
        pdf += f"{offset:010} 00000 n \n".encode("ascii")
    pdf += (
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n"
        f"startxref\n{xref}\n%%EOF\n"
    ).encode("ascii")
    return bytes(pdf)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run_case(source: bytes, rotation: int, crop_box: list[int], source_path: Path) -> dict:
    source_path.write_bytes(source)
    source_hash = sha256(source)
    output = subprocess.run(
        [
            str(DRIVER),
            str(APK_DIR / "libSPenModel.so"),
            "--pdf-paper-body-writer-matrix",
            str(APK_DIR / "libSPenPdfiumB.so"),
            str(APK_DIR / "libSPenComposer.so"),
            str(APK_DIR / "libSPenBase.so"),
            str(source_path),
            source_hash,
            str(NOTE_PAGE[0]),
            str(NOTE_PAGE[1]),
            "/tmp/sdocx-pdf-body-writer-matrix-unused.pdf",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    if output.returncode:
        raise SystemExit(f"native driver failed ({output.returncode}): {output.stderr}")
    native = json.loads(output.stdout)
    if native["source_rotation_quarter_turns"] != rotation // 90:
        raise SystemExit(f"native rotation differs from fixture source: {native}")
    if native["loaded_page_rect"] != [0, 0, 115, 150]:
        raise SystemExit(f"native page dimensions differ for CropBox/UserUnit input: {native}")
    if native["get_object_matrix_result"] != 1 or native["get_object_position_result"] != 1:
        raise SystemExit(f"native placement helper failed: {native}")
    scale = 115 / NOTE_PAGE[0]
    expected_body_rect = [
        (MARKER["rect"][0] + MARKER["position"][0]) * scale,
        (MARKER["rect"][1] + MARKER["position"][1]) * scale,
        (MARKER["rect"][2] + MARKER["position"][0]) * scale,
        (MARKER["rect"][3] + MARKER["position"][1]) * scale,
    ]
    body_rect = native["body_writer_draw_object_rect"]
    if len(body_rect) != 4 or len(expected_body_rect) != 4:
        raise SystemExit(f"Composer BodyText rectangle must contain four coordinates: {native}")
    if len(native["matrix_input_rect"]) != 4 or native["matrix_input_rect"] != body_rect:
        raise SystemExit(f"PDFium matrix input must equal the captured BodyText rectangle: {native}")
    if abs(native["native_write_body_text_scale"] - scale) > 0.000001:
        raise SystemExit(f"native WriteBodyText scale differs from host expectation: {native}")
    if abs(native["host_expected_note_scale"] - scale) > 0.000001:
        raise SystemExit(f"host expected scale provenance changed: {native}")
    if any(abs(actual - expected) > 0.001 for actual, expected in zip(body_rect, expected_body_rect)):
        raise SystemExit(f"Composer BodyText writer output changed: {native}")
    native["source_pdf_sha256"] = source_hash
    native["host_generated_source_pdf"] = {
        "media_box": MEDIA_BOX,
        "crop_box": crop_box,
        "rotate_degrees": rotation,
        "user_unit": USER_UNIT,
        "source_vector_path": "17 21 m 91 44 l 57 118 l h 0 0 1 rg f",
    }
    native["interpretation"] = (
        "The source PDF parser reports CropBox dimensions 150x115, rotated to 115x150; "
        "the factor UserUnit=2 is not present in those dimensions. The paired offset-CropBox "
        "case shifts native PDFium matrix and position translations by its origin delta. "
        "Composer transforms only the supplied note marker and imported page rectangle; "
        "the final native PDFium helper call is separate from Drawing::DrawObject."
    )
    return native


def capture() -> dict:
    for name, expected in LIBRARY_HASHES.items():
        actual = sha256((APK_DIR / name).read_bytes())
        if actual != expected:
            raise SystemExit(f"pinned APK library hash changed for {name}: {actual}")
    subprocess.run(
        [
            "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
            "conformance/native_table.rs", "-L", f"native={UNICORN_LIB}",
            "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}", "-o", str(DRIVER),
        ],
        cwd=ROOT,
        check=True,
    )
    cases = []
    with tempfile.TemporaryDirectory(prefix="sdocx-pdf-body-writer-") as directory:
        temp = Path(directory)
        for rotation in (90, 270):
            for crop_box in ([0, 5, 150, 120], [10, 20, 160, 135]):
                source = source_pdf(rotation, crop_box)
                filename = f"source-{rotation}-{crop_box[0]}-{crop_box[1]}.pdf"
                cases.append(run_case(source, rotation, crop_box, temp / filename))
    for rotation in (90, 270):
        base = next(case for case in cases if case["source_rotation_quarter_turns"] == rotation // 90 and case["host_generated_source_pdf"]["crop_box"][0] == 0)
        shifted = next(case for case in cases if case["source_rotation_quarter_turns"] == rotation // 90 and case["host_generated_source_pdf"]["crop_box"][0] == 10)
        for index, delta in ((4, 10), (5, 15)):
            if abs((shifted["matrix"][index] - base["matrix"][index]) - delta) > 0.001:
                raise SystemExit(f"PDFium native matrix translation did not follow CropBox origin for {rotation}: {base}, {shifted}")
        for index, delta in ((0, 10), (1, 15), (2, 10), (3, 15)):
            if abs((shifted["position_rect"][index] - base["position_rect"][index]) - delta) > 0.001:
                raise SystemExit(f"PDFium native position did not follow CropBox origin for {rotation}: {base}, {shifted}")
    return {
        "schema_version": 1,
        "apk": {"version": "4.4.45.37", "sha256": "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"},
        "native_library_sha256": LIBRARY_HASHES,
        "note_page_wh_host_input": NOTE_PAGE,
        "synthetic_drawn_text_marker_host_input": MARKER,
        "page_rectangle_boundary": "Host supplies rectangle origin [0,0]; native PDFium provides the loaded page width and height used for its right/bottom edges.",
        "source_geometry_boundary": "Host generator supplies MediaBox/CropBox/Rotate/UserUnit and asymmetric source vector; pinned native PdfiumImpl parses dimensions/rotation from the actual generated PDF.",
        "matrix_composition_boundary": "Pinned native Composer executes addBodyText and BodyTextPDFWriter through RectF Offset/Scale into a host Drawing::DrawObject sink; a separate pinned native PdfiumImpl GetObjectMatrix/GetObjectPosition call consumes that captured rect. No full DrawObject-to-AddImage dispatch, FPDF insertion, content generation, serialization, or pixels are captured.",
        "cases": cases,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    encoded = json.dumps(capture(), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native BodyText/PDFium matrix capture differs from conformance/pdf-paper-body-writer-matrix.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
