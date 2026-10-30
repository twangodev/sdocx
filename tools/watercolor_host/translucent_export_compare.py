"""Compare native premultiplied frames with public canvas and export readers."""

import hashlib
import json
from pathlib import Path
import struct

import pymupdf
import pypdfium2
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
DIRECTORY = Path("/tmp/sdocx-watercolor-translucent")
NATIVE = ROOT / "conformance/watercolor-translucent-source-native.json"
fixture = json.loads(NATIVE.read_text())


def premultiply(raw):
    result = bytearray(raw)
    for i in range(0, len(result), 4):
        a = result[i + 3]
        for c in range(3):
            result[i + c] = (result[i + c] * a + 127) // 255
    return bytes(result)


def metrics(actual, expected):
    assert len(actual) == len(expected)
    differences = [abs(a - b) for a, b in zip(actual, expected)]
    return {
        "different_channels": sum(d != 0 for d in differences),
        "maximum": max(differences),
        "absolute_sum": sum(differences),
    }


results = {}
for name, case in fixture["cases"].items():
    w, h = case["frame"]
    bottom = bytearray(bytes(case["background"]) * (w * h))
    packed = bytes.fromhex(case["final_sparse_hex"])
    for offset in range(0, len(packed), 8):
        index = struct.unpack_from("<I", packed, offset)[0]
        bottom[index * 4 : index * 4 + 4] = packed[offset + 4 : offset + 8]
    assert hashlib.sha256(bottom).hexdigest() == case["final_sha256"]
    expected = b"".join(bottom[y * w * 4 : (y + 1) * w * 4] for y in reversed(range(h)))
    outputs = {
        "canvas": (DIRECTORY / (name + ".rgba")).read_bytes(),
        "png": premultiply(
            Image.open(DIRECTORY / (name + ".png")).convert("RGBA").tobytes()
        ),
    }
    for route, suffix in [("svg_pdf", ".pdf"), ("direct_pdf", ".direct.pdf")]:
        path = DIRECTORY / (name + suffix)
        with pymupdf.open(path) as doc:
            pix = doc[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
            assert (pix.width, pix.height, pix.n) == (w, h, 4)
            outputs[route + "_mupdf"] = bytes(pix.samples)
        with pypdfium2.PdfDocument(str(path)) as doc:
            page = doc[0]
            bitmap = page.render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0))
            outputs[route + "_pdfium"] = premultiply(
                bitmap.to_pil().convert("RGBA").tobytes()
            )
            bitmap.close()
            page.close()
    results[name] = {key: metrics(value, expected) for key, value in outputs.items()}
    print(name, results[name], flush=True)
receipt = {
    "schema": "sdocx-watercolor-translucent-export-v1",
    "native_receipt_sha256": hashlib.sha256(NATIVE.read_bytes()).hexdigest(),
    "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "readers": {"mupdf": pymupdf.VersionBind, "pdfium": str(pypdfium2.PDFIUM_INFO)},
    "dpi": 72,
    "scope": "Eight synthetic GLV1/GLV2 host-Mesa controls over transparent and translucent premultiplied clear colors at 1x and 2x. All comparisons are premultiplied RGBA; PNG and PDFium outputs are premultiplied after reading. Not saved-page or device fidelity.",
    "results": results,
}
(ROOT / "conformance/watercolor-translucent-export.json").write_text(
    json.dumps(receipt, indent=2) + "\n"
)
