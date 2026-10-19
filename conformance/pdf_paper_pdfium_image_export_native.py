"""Execute and check a native one-page StandardPDF PNG-plus-vector overlay.

Run with the project conformance venv and explicit retained inputs:
  conformance/.venv/bin/python conformance/pdf_paper_pdfium_image_export_native.py \
    --source-pdf /tmp/quiz.pdf --overlay-png /tmp/q.png --write-fixture
  conformance/.venv/bin/python conformance/pdf_paper_pdfium_image_export_native.py \
    --source-pdf /tmp/quiz.pdf --overlay-png /tmp/q.png --check-fixture
"""
import argparse
import hashlib
import json
import math
import subprocess
from pathlib import Path

import pymupdf

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-paper-pdfium-image-export.json"
DRIVER = Path("/tmp/sdocx-pdf-paper-image-export-conformance")
OUTPUT = Path("/tmp/sdocx-pdf-paper-image-export.pdf")
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"
LIBCPP_SHA256 = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"
SPENPDF_SHA256 = "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04"
SPENBASE_SHA256 = "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
LIBPNG_SHA256 = "898abcc6ddbe3a4ad7e076484df666b732913db95a7abc7d6e99bfd835f555eb"
LIBM_PATH = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")
LIBM_SHA256 = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"
LIBZ_SHA256 = "85590dd58edf5445e18bc7193e5ebc01ac5841f1ae187e97705a662e90c6421e"
SOURCE_SHA256 = "f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279"
OVERLAY_PNG_SHA256 = "6aea666d9d95f1b887f9c777e1ef451208b666f69797d5c2bf4fa113bd44ea0d"
PAGE_SCALE = 0.400775
NOTE_RECT = (100.0, 100.0, 300.0, 300.0)
NOTE_PATH = ((100.0, 100.0), (300.0, 100.0), (200.0, 350.0))


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def close_tuple(actual, expected, tolerance: float, label: str) -> None:
    values = tuple((float(a), float(e)) for a, e in zip(actual, expected))
    if len(actual) != len(expected) or any(not math.isfinite(a) or not math.isfinite(e) or abs(a - e) > tolerance for a, e in values):
        raise SystemExit(f"{label} differs: actual={tuple(actual)} expected={tuple(expected)} tolerance={tolerance}")


def verify_inputs(source: Path, overlay: Path) -> None:
    for path, expected, label in [
        (source, SOURCE_SHA256, "source PDF"),
        (overlay, OVERLAY_PNG_SHA256, "overlay PNG"),
        (APK_DIR / "libSPenModel.so", MODEL_SHA256, "model library"),
        (APK_DIR / "libSPenPdfiumB.so", PDFIUM_SHA256, "PDFium library"),
        (APK_DIR / "libc++_shared.so", LIBCPP_SHA256, "libc++ library"),
        (APK_DIR / "libSPenPdf.so", SPENPDF_SHA256, "Samsung PDF library"),
        (APK_DIR / "libSPenBase.so", SPENBASE_SHA256, "Samsung base library"),
        (APK_DIR / "libSPenLibpng.so", LIBPNG_SHA256, "Samsung PNG library"),
        (LIBM_PATH, LIBM_SHA256, "host libm"),
    ]:
        actual = sha(path)
        if actual != expected:
            raise SystemExit(f"{label} SHA-256 changed: {actual}")
    if len(str(overlay).encode("ascii")) > 22:
        raise SystemExit("the native short-path field requires an ASCII overlay path no longer than 22 bytes")


def capture(source: Path, overlay: Path) -> dict:
    verify_inputs(source, overlay)
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN_LIB}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)

    OUTPUT.unlink(missing_ok=True)
    run = subprocess.run([
        str(DRIVER), str(APK_DIR / "libSPenModel.so"),
        "--pdfium-standard-export-image", str(APK_DIR / "libSPenPdfiumB.so"),
        str(source), SOURCE_SHA256, str(overlay), OVERLAY_PNG_SHA256, str(OUTPUT),
    ], cwd=ROOT, text=True, capture_output=True, check=True)
    native = json.loads(run.stdout)
    if native.get("source_pdf_sha256") != SOURCE_SHA256 or native.get("overlay_png_sha256") != OVERLAY_PNG_SHA256:
        raise SystemExit("native capture source hashes do not match the verified inputs")
    if native.get("export_executed") is not True or native.get("export_result") != 0:
        raise SystemExit("native export did not report an executed successful save")
    if native.get("writer_open") != 1 or native.get("writer_close") != 1:
        raise SystemExit("native writer open/close failed")
    if native.get("image_add_result") != 1 or native.get("image_generate_content_result") != 1:
        raise SystemExit("native image insertion/content generation failed")
    if OUTPUT.stat().st_size != native.get("export_output_bytes"):
        raise SystemExit("exported file byte count differs from the native writer report")
    if sha(OUTPUT) != native.get("export_output_sha256"):
        raise SystemExit("exported file bytes do not match the native output hash")

    source_doc = pymupdf.open(source)
    output_doc = pymupdf.open(OUTPUT)
    if len(source_doc) != 1 or len(output_doc) != 1:
        raise SystemExit("expected a single retained source page and a single output page")
    source_page, output_page = source_doc[0], output_doc[0]
    close_tuple(tuple(output_page.rect), tuple(source_page.rect), 0.001, "retained page rectangle")
    close_tuple(tuple(native["loaded_page_geometry"]), (source_page.rect.width, source_page.rect.height), 0.001, "native loaded page geometry")

    source_images = source_page.get_image_info(xrefs=True)
    output_images = output_page.get_image_info(xrefs=True)
    if len(source_images) != 1 or len(output_images) != 2:
        raise SystemExit(f"expected one retained source image and one added overlay image: source={len(source_images)} output={len(output_images)}")
    original_image = next((item for item in output_images if item["width"] == source_images[0]["width"] and item["height"] == source_images[0]["height"]), None)
    overlay_image = next((item for item in output_images if item["width"] == 1 and item["height"] == 1), None)
    if original_image is None or overlay_image is None:
        raise SystemExit("could not distinguish retained source image from the 1x1 overlay image")
    if original_image["digest"] != source_images[0]["digest"]:
        raise SystemExit("retained source-page image pixels changed")
    overlay_refs = next(item for item in output_page.get_images(full=True) if item[0] == overlay_image["xref"])
    color = pymupdf.Pixmap(output_doc, overlay_image["xref"])
    mask_xref = overlay_refs[1]
    if mask_xref:
        overlay_pixmap = pymupdf.Pixmap(color, pymupdf.Pixmap(output_doc, mask_xref))
    else:
        overlay_pixmap = color
    input_pixmap = pymupdf.Pixmap(overlay.read_bytes())
    if (overlay_pixmap.width, overlay_pixmap.height, overlay_pixmap.n, overlay_pixmap.samples) != (
        input_pixmap.width, input_pixmap.height, input_pixmap.n, input_pixmap.samples
    ):
        raise SystemExit("serialized overlay image pixels differ from the input PNG")

    overlay_bbox = tuple(overlay_image["bbox"])
    scale = float(native["page_scale"])
    expected_bbox = (NOTE_RECT[0] * scale, NOTE_RECT[1] * scale, NOTE_RECT[2] * scale, NOTE_RECT[3] * scale)
    close_tuple(overlay_bbox, expected_bbox, 0.1, "native image placement bbox")

    expected_path = tuple((x * scale, y * scale) for x, y in NOTE_PATH)
    expected_rect = (min(p[0] for p in expected_path), min(p[1] for p in expected_path), max(p[0] for p in expected_path), max(p[1] for p in expected_path))
    drawings = output_page.get_drawings()
    drawing = next((item for item in drawings if all(abs(a - b) <= 0.002 for a, b in zip(tuple(item["rect"]), expected_rect))), None)
    if drawing is None:
        raise SystemExit(f"native overlay path bounds not found: expected={expected_rect}")
    actual_path = []
    for operation in drawing["items"]:
        if operation[0] != "l":
            raise SystemExit(f"unexpected output path operation: {operation[0]}")
        start, end = operation[1], operation[2]
        if not actual_path:
            actual_path.append((start.x, start.y))
        actual_path.append((end.x, end.y))
    expected_closed = (*expected_path, expected_path[0])
    close_tuple(tuple(value for point in actual_path for value in point), tuple(value for point in expected_closed for value in point), 0.002, "native path vertices")

    if native.get("host_zlib_sha256") != LIBZ_SHA256:
        raise SystemExit("host zlib hash differs")
    zlib_streams = native.get("inflate_streams", [])
    if len(zlib_streams) != 2 or [item["host_bridge"] for item in zlib_streams] != ["host_zeroed_alloc_free", "host_alloc_free"]:
        raise SystemExit(f"expected the PDFium and pinned libpng allocator bridges: {zlib_streams}")
    for key in ("inflateInit2_", "inflate", "inflateEnd", "crc32"):
        if key not in native.get("handled_imports", []):
            raise SystemExit(f"native PNG decode did not execute {key}")

    return {
        "source_pdf_sha256": SOURCE_SHA256,
        "overlay_png_sha256": OVERLAY_PNG_SHA256,
        "native_libraries": {
            "model": MODEL_SHA256,
            "pdfium": PDFIUM_SHA256,
            "libcxx": LIBCPP_SHA256,
            "samsung_pdf": SPENPDF_SHA256,
            "samsung_base": SPENBASE_SHA256,
            "samsung_libpng": LIBPNG_SHA256,
            "host_libm": LIBM_SHA256,
            "host_zlib": LIBZ_SHA256,
        },
        "native_execution": {
            "path": native["native_path"],
            "loaded_page_geometry": native["loaded_page_geometry"],
            "source_rotation": native["source_rotation"],
            "writer_open": native["writer_open"],
            "image_add_result": native["image_add_result"],
            "image_generate_content_result": native["image_generate_content_result"],
            "writer_close": native["writer_close"],
            "export_executed": native["export_executed"],
            "export_result": native["export_result"],
            "host_zlib_streams": zlib_streams,
            "decode_produced_bytes": native["inflate_total_produced_bytes"],
        },
        "output": {
            "sha256": sha(OUTPUT),
            "bytes": OUTPUT.stat().st_size,
            "page_rect": list(output_page.rect),
            "retained_source_image_digest": source_images[0]["digest"].hex(),
            "overlay_pixel_rgba_hex": overlay_pixmap.samples.hex(),
            "overlay_bbox": list(overlay_bbox),
            "overlay_bbox_expected_from_note_rect_and_scale": list(expected_bbox),
            "vector_path_vertices": [list(point) for point in actual_path],
        },
        "host_boundaries": {
            "source_and_overlay_files": "external inputs admitted only at their pinned SHA-256 values",
            "image_rect_and_path": "synthetic note-space inputs supplied by this harness; the native AddImage and DrawPath handlers execute",
            "PNG": "pinned Samsung libSPenLibpng executes; inflate/checksum calls use pinned host zlib with bounded allocators",
            "filesystem": "host file callbacks and explicit deterministic platform callbacks",
            "full_app_route": "Composer/body-writer coordinate transform and Android JNI route are not executed",
            "pixel_scope": "retained source image digest and overlay PNG pixel equality are checked; arbitrary crop/outside-crop rendering is not claimed",
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-pdf", type=Path, required=True)
    parser.add_argument("--overlay-png", type=Path, required=True)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    encoded = json.dumps(capture(args.source_pdf, args.overlay_png), indent=2) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native PDFium PNG-plus-vector export differs from conformance/pdf-paper-pdfium-image-export.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
