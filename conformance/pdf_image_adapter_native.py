"""Replay the pinned native PdfImageAdapter String-overload into PDFium AddImage.

Run from the repository root with the extracted APK and bundled Unicorn runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_image_adapter_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pdf_image_adapter_native.py --check-fixture
"""
import argparse
import hashlib
import json
import re
import subprocess
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-image-adapter-native.json"
DRIVER = Path("/tmp/sdocx-pdf-image-adapter-native")
UNICORN = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
LIBM = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")
PINS = {
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenPdfiumB.so": "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854",
    "libSPenPdf.so": "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
    "libSPenLibpng.so": "898abcc6ddbe3a4ad7e076484df666b732913db95a7abc7d6e99bfd835f555eb",
}
PNG = bytes.fromhex(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
    "0000000d49444154789c63f8cfc0f01f00050001ff89993d1d0000000049454e44ae426082"
)
DESTINATION_RECT = [33.0, 37.0, 63.0, 77.0]
SOURCE_CLIP = [0.0, 0.0, 1.0, 1.0]
SCALE = 0.06346578


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_pdf(rotation: int, crop_x: int, crop_y: int) -> bytes:
    media = [-20, -10, 180, 140]
    crop = [crop_x, crop_y, crop_x + 150, crop_y + 115]
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        (f"<< /Type /Page /Parent 2 0 R /MediaBox [{media[0]} {media[1]} {media[2]} {media[3]}] "
         f"/CropBox [{crop[0]} {crop[1]} {crop[2]} {crop[3]}] /Rotate {rotation} /UserUnit 2 "
         "/Resources << >> /Contents 4 0 R >>").encode("ascii"),
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
    pdf += f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode("ascii")
    return bytes(pdf)


def object_bodies(pdf: bytes):
    return [(int(m.group(1)), m.group(2)) for m in re.finditer(rb"(?m)^(\d+) 0 obj\s*(.*?)\s*endobj", pdf, re.S)]


def decoded_stream(body: bytes) -> bytes | None:
    m = re.search(rb"stream\r?\n(.*?)\r?\nendstream", body, re.S)
    if not m:
        return None
    raw = m.group(1)
    return zlib.decompress(raw) if b"/FlateDecode" in body else raw


def output_evidence(output_path: Path, rotation: int, crop_x: int, crop_y: int) -> dict:
    pdf = output_path.read_bytes()
    if not pdf.startswith(b"%PDF-") or b"%%EOF" not in pdf:
        raise SystemExit("native output is not a complete PDF")
    bodies = object_bodies(pdf)
    pages = [(number, body) for number, body in bodies if re.search(rb"/Type\s*/Page(?:\s|/)", body) and not re.search(rb"/Type\s*/Pages\b", body)]
    if len(pages) != 1:
        raise SystemExit(f"expected one serialized source page, got {len(pages)}")
    page_number, page_body = pages[0]
    crop = [crop_x, crop_y, crop_x + 150, crop_y + 115]
    for value in [
        f"/Rotate {rotation}".encode(),
        b"/MediaBox[ -20 -10 180 140]",
        f"/CropBox[ {crop[0]} {crop[1]} {crop[2]} {crop[3]}]".encode(),
        b"/UserUnit 2",
    ]:
        if value not in page_body:
            raise SystemExit(f"serialized page lost source field {value!r}")
    streams = [decoded_stream(body) for _, body in bodies]
    streams = [s for s in streams if s is not None]
    if not any(b"17 21 m 91 44 l 57 118 l h 0 0 1 rg f" in s for s in streams):
        raise SystemExit("original source vector content was not retained")
    image_objects = [(number, body) for number, body in bodies if b"/Subtype/Image" in body]
    if not image_objects:
        raise SystemExit("serialized output has no embedded image object")
    image_object_numbers = {number for number, body in image_objects}
    image_resource_names = {
        name for name, number in re.findall(rb"/([A-Za-z0-9]+)\s+(\d+)\s+0\s+R", page_body)
        if int(number) in image_object_numbers
    }
    image_matrices = []
    for stream in streams:
        for name in image_resource_names:
            pattern = rb"([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+cm\s+/" + re.escape(name) + rb"\s+Do"
            image_matrices.extend([[float(value) for value in match.groups()] for match in re.finditer(pattern, stream)])
    if len(image_matrices) != 1:
        raise SystemExit(f"expected one image placement matrix, got {len(image_matrices)}")
    return {
        "page_object": page_number,
        "page_count": 1,
        "source_page_fields_retained": {"media_box": [-20, -10, 180, 140], "crop_box": crop, "rotate_degrees": rotation, "user_unit": 2},
        "source_vector_content_retained": True,
        "image_xobject_count_including_masks": len(image_objects),
        "image_placement_matrices_from_serialized_streams": image_matrices,
        "output_bytes": len(pdf),
        "output_sha256": sha(pdf),
    }


def capture_case(index: int, rotation: int, crop_x: int, crop_y: int) -> dict:
    source = Path(f"/tmp/sdocx-pia-source-{index}.pdf")
    # Keep the native short path within its 11-UTF-16-unit storage and avoid
    # the generic /tmp/i.png used by other probes.
    png = Path(f"/tmp/sdpia{index}.png")
    output = Path(f"/tmp/sdocx-pia-output-{index}.pdf")
    source_bytes = source_pdf(rotation, crop_x, crop_y)
    source.write_bytes(source_bytes)
    png.write_bytes(PNG)
    args = [
        str(DRIVER), str(APK / "libSPenModel.so"), "--pdf-image-adapter-native",
        str(APK / "libSPenPdfiumB.so"), str(APK / "libSPenComposer.so"),
        str(source), sha(source_bytes), str(png), sha(PNG),
        *[str(value) for value in DESTINATION_RECT],
        *[str(value) for value in SOURCE_CLIP], str(SCALE), str(output),
    ]
    run = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, check=True)
    native = json.loads(run.stdout)
    if native["image_adapter"] is None:
        raise SystemExit("native adapter capture missing")
    for field in ("native_constructor", "native_set_image_string_rect", "native_set_rect", "native_set_scale", "info_pointer_passed_to_add_image"):
        if native["image_adapter"].get(field) is not True:
            raise SystemExit(f"native adapter evidence missing {field}")
    adapter = native["image_adapter"]
    if adapter["info_pointer"] != adapter["add_image_entry_info_pointer"]:
        raise SystemExit("native AddImage entry did not receive the constructor-owned info pointer")
    for field in ("constructor_info_128_hex", "after_setters_info_128_hex"):
        if len(adapter[field]) != 256:
            raise SystemExit(f"native adapter {field} is not a complete 128-byte snapshot")
    if adapter["constructor_info_128_hex"] == adapter["after_setters_info_128_hex"]:
        raise SystemExit("native setter calls did not change the constructor-owned info block")
    if adapter["path_provenance"] != "host input; native path field not decoded":
        raise SystemExit("native adapter path provenance is not explicit")
    if native["export_executed"] is not True or native["export_result"] != 0 or native["writer_close"] != 1 or native["image_add_result"] != 1 or native["image_generate_content_result"] != 1:
        raise SystemExit("native AddImage/GenerateContent/export path did not complete")
    parsed = output_evidence(output, rotation, crop_x, crop_y)
    native.update({
        "case": f"rotate_{rotation}_crop_origin_{crop_x}_{crop_y}",
        "source_pdf_sha256": sha(source_bytes),
        "source_pdf_bytes": len(source_bytes),
        "source_png_sha256": sha(PNG),
        "source_png_bytes": len(PNG),
        "host_inputs": {"destination_rect": DESTINATION_RECT, "source_clip": SOURCE_CLIP, "scale": SCALE},
        "native_adapter_output": native["image_adapter"],
        "serialized_output": parsed,
        "host_boundaries": {
            "source_fixture": "stdlib-generated PDF with asymmetric vector source content, CropBox, Rotate and UserUnit",
            "image_bytes": "deterministic host-generated 1x1 RGBA PNG admitted by SHA-256",
            "adapter_inputs": "destination rect, source clip and scale are explicit host inputs; they are not derived from BodyText dispatch",
            "native_calls": "Composer adapter constructor, SetImage(String,RectF), SetRect, SetScale; PdfiumImageHandler::AddImage, content generation, writer close, Export and SaveAsCopy",
            "bitmap_overload": "SetImage(ISPBitmap*) and raster-batch production are not executed",
        },
    })
    native.pop("image_adapter")
    return native


def capture() -> dict:
    for name, expected in PINS.items():
        actual = sha((APK / name).read_bytes())
        if actual != expected:
            raise SystemExit(f"pinned DSO changed: {name} {actual}")
    cases = [(r, 0, 5) for r in (0, 90, 180, 270)] + [(90, 10, 20), (270, 10, 20)]
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN}", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    captured = [capture_case(index, *case) for index, case in enumerate(cases)]
    by_case = {item["case"]: item for item in captured}
    for rotation in (90, 270):
        base = by_case[f"rotate_{rotation}_crop_origin_0_5"]["serialized_output"]["image_placement_matrices_from_serialized_streams"][0]
        shifted = by_case[f"rotate_{rotation}_crop_origin_10_20"]["serialized_output"]["image_placement_matrices_from_serialized_streams"][0]
        if any(abs(base[index] - shifted[index]) > 2e-5 for index in range(4)):
            raise SystemExit(f"CropBox origin shift changed image linear matrix at rotation {rotation}")
        if abs((shifted[4] - base[4]) - 10.0) > 1e-4 or abs((shifted[5] - base[5]) - 15.0) > 1e-4:
            raise SystemExit(f"CropBox origin translation delta changed at rotation {rotation}")
    return {
        "schema": "sdocx.pdf-image-adapter-native.v1",
        "pinned_dso_sha256": PINS,
        "shifted_crop_origin_translation_delta": [10.0, 15.0],
        "cases": captured,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    actual = capture()
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(actual, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0
    expected = json.loads(FIXTURE.read_text())
    if actual != expected:
        raise SystemExit("native PDF image adapter capture differs from fixture")
    print("PDF image adapter native fixture matches")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
