"""Compare saved-scene PDF exports with their native capture pixels.

Run the saved_scene_candidate_replays_original_native_frames test with
SDOCX_FOUNTAIN_SCENE_ARTIFACTS set to the directory passed as argv[1].
"""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
directory = Path(sys.argv[1])
native = root / "crates/sdocx/tests/assets/fountain-v14/scene-native.json"
cases = json.loads(native.read_bytes())["cases"]
results = {}
expected_names = set(cases)
assert {path.stem for path in directory.glob("*.rgba")} == expected_names


def compare(expected, rendered):
    assert len(expected) == len(rendered)
    errors = [abs(a - b) for a, b in zip(expected, rendered)]
    maximum = max(errors, default=0)
    assert maximum <= 1, maximum
    return {"sha256": hashlib.sha256(rendered).hexdigest(), "max_channel_error": maximum,
            "differing_channels": sum(error != 0 for error in errors), "absolute_error": sum(errors)}

for rgba in sorted(directory.glob("*.rgba")):
    expected = rgba.read_bytes()
    assert len(expected) == 128 * 256 * 4
    output = cases[rgba.stem]["output"]
    captured = zlib.decompress(base64.b64decode(output["rgba_zlib_base64"]))
    assert hashlib.sha256(captured).hexdigest() == output["sha256"]
    captured = b"".join(captured[y * 512 : (y + 1) * 512] for y in reversed(range(256)))
    assert expected == captured, rgba.stem
    metrics = {}
    artifact_hashes = {"rgba_sha256": hashlib.sha256(expected).hexdigest()}
    for route in ("svg", "direct"):
        path = rgba.with_suffix(f".{route}.pdf")
        artifact_hashes[f"{route}_pdf_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        with pymupdf.open(path) as document:
            image = document[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
            assert (image.width, image.height, image.n) == (128, 256, 4)
            rendered = bytes(image.samples)
            metrics[f"{route}_mupdf"] = compare(expected, rendered)
        with pypdfium2.PdfDocument(str(path)) as document:
            page = document[0]
            bitmap = page.render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0))
            image = bitmap.to_pil().convert("RGBA")
            assert image.size == (128, 256)
            straight = image.tobytes()
            rendered = bytes(channel if i % 4 == 3 else (channel * straight[i - i % 4 + 3] + 127) // 255 for i, channel in enumerate(straight))
            metrics[f"{route}_pdfium"] = compare(expected, rendered)
            bitmap.close()
            page.close()
    results[rgba.stem] = {"native_topdown_sha256": hashlib.sha256(captured).hexdigest(), "artifacts": artifact_hashes, "readers": metrics}
assert len(expected_names) == 108 and set(results) == expected_names
receipt = {
    "schema": "sdocx-fountain-v14-saved-scene-export-v1",
    "scope": "Explicit saved-page stroke submissions and caller-managed scene keys replay the captured scene in native capture order. All native frames match SVG rasterization exactly. Both PDF routes are measured in MuPDF premultiplied RGBA and PDFium straight RGBA converted to premultiplied using rounded integer multiplication. Reader results are bounded to one channel level; this does not establish automatic saved-page dispatch or device fidelity.",
    "native_receipt_sha256": hashlib.sha256(native.read_bytes()).hexdigest(),
    "comparator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "readers": {"mupdf": pymupdf.VersionBind, "pdfium": str(pypdfium2.PDFIUM_INFO)},
    "dpi": 72,
    "results": results,
}
output = root / "conformance/fountain-v14-saved-scene-export.json"
output.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
print(f"{len(results)} frames compared through both PDF routes in both readers: {output}")
