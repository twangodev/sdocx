"""Compare the fixed-grid experiment's public SVG-page PDF readers with native."""

import hashlib
import json
import sys
import zlib
from pathlib import Path
import pymupdf
import pypdfium2
from PIL import Image

svg, png, pdf = map(Path, sys.argv[1:])
raw = zlib.decompress(
    Path("tools/crayon2-independent-control/native-frame.z").read_bytes()
)
assert (
    hashlib.sha256(raw).hexdigest()
    == "2dc7558f45bac784ad1ba3d5e40f028ad5c7a4e96f80e1462f37930f9f742a7f"
)
rows = b"".join(raw[y * 768 : (y + 1) * 768] for y in reversed(range(192)))
native = bytes(v for i, v in enumerate(rows) if i % 4 != 3)
with Image.open(png) as image:
    assert image.size == (192, 192)
    frames = {"resvg": image.convert("RGB").tobytes()}
with pymupdf.open(pdf) as doc:
    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=False)
    assert (pix.width, pix.height) == (192, 192)
    frames["mupdf"] = pix.samples
with pypdfium2.PdfDocument(pdf) as doc:
    img = doc[0].render(scale=4 / 3).to_pil().convert("RGB")
    assert img.size == (192, 192)
    frames["pdfium"] = img.tobytes()
metrics = {}
for name, frame in frames.items():
    diff = [abs(a - b) for a, b in zip(native, frame, strict=True)]
    metrics[name] = [
        sum(any(diff[i : i + 3]) for i in range(0, len(diff), 3)),
        max(diff),
        sum(diff),
    ]
result = {
    "scope": "fixed 192x192 host output grid; public SVG-page PDF; no device, saved page, rescaling or direct document PDF claim",
    "metric_order": ["differing_pixels", "maximum_channel_error", "absolute_rgb_error"],
    "readers_vs_native": metrics,
    "sha256": {
        name: hashlib.sha256(path.read_bytes()).hexdigest()
        for name, path in (("svg", svg), ("png", png), ("pdf", pdf))
    },
}
print(json.dumps(result, indent=2))
