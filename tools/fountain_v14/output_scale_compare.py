import json
import base64
import zlib
import subprocess
import sys
import hashlib
from pathlib import Path
from PIL import Image
import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
out = Path(sys.argv[1])
r = json.loads(
    (root / "conformance/fountain-v14-drawing-paint-native.json").read_text()
)
result = {}
for variant in ["baseline", "candidate"]:
    for scale in [1, 2]:
        case = r["cases"][f"drawing_center_top_{scale}_ff000000_white"]
        raw = zlib.decompress(base64.b64decode(case["output"]["rgba_zlib_base64"]))
        assert len(raw) == 64 * 64 * 4
        assert hashlib.sha256(raw).hexdigest() == case["output"]["sha256"]
        expected = b"".join(raw[y * 256 : (y + 1) * 256] for y in reversed(range(64)))
        stem = out / variant / str(scale)
        png = stem.with_suffix(".png")
        subprocess.run(
            ["resvg", "-z", str(scale), str(stem.with_suffix(".svg")), str(png)],
            check=True,
            capture_output=True,
        )
        images = {"svg": Image.open(png).convert("RGBA")}
        doc = pymupdf.open(stem.with_suffix(".pdf"))
        assert not doc[0].get_images()
        pix = doc[0].get_pixmap(
            matrix=pymupdf.Matrix(scale * 4 / 3, scale * 4 / 3), alpha=False
        )
        images["mupdf"] = Image.frombytes(
            "RGB", (pix.width, pix.height), pix.samples
        ).convert("RGBA")
        pdf = pypdfium2.PdfDocument(stem.with_suffix(".pdf"))
        images["pdfium"] = pdf[0].render(scale=scale * 4 / 3).to_pil().convert("RGBA")
        x, y = case["source_origin"]
        box = (x * scale, y * scale, x * scale + 64, y * scale + 64)
        for reader, im in images.items():
            assert im.size == (1024 * scale, 1536 * scale)
            actual = im.crop(box).tobytes()
            assert len(actual) == len(expected)
            result[f"{variant}-{scale}-{reader}"] = sum(
                abs(a - b) for a, b in zip(actual, expected)
            )
        png.unlink()
        doc.close()
        pdf.close()
receipt = {
    "scope": "One original saved Fountain V14 source, direct host paint over white at 1x and 2x. No device or general saved-page parity.",
    "baseline_commit": "28f20dcf",
    "native_fixture": "conformance/fountain-v14-drawing-paint-native.json",
    "native_sha256": hashlib.sha256(
        (root / "conformance/fountain-v14-drawing-paint-native.json").read_bytes()
    ).hexdigest(),
    "source_fixture_sha256": hashlib.sha256(
        (root / "conformance/fountain-v14-scaled-source-rgba-native.json").read_bytes()
    ).hexdigest(),
    "readers": {
        "resvg": subprocess.check_output(["resvg", "--version"], text=True).strip(),
        "pymupdf": pymupdf.VersionBind,
        "pypdfium2": str(pypdfium2.PYPDFIUM_INFO),
        "pdfium": str(pypdfium2.PDFIUM_INFO),
    },
    "measurement": "Absolute RGBA8 channel error; opaque white, top-origin 64x64 crop at source_origin multiplied by scale; direct PDF at scale*96 DPI, no image objects.",
    "results": result,
    "artifacts_sha256": {
        str(p.relative_to(out)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(out.glob("*/*"))
        if p.suffix in (".svg", ".pdf")
    },
    "comparator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "candidate_source_sha256": {
        name: hashlib.sha256((Path(sys.argv[3]) / name).read_bytes()).hexdigest()
        for name in [
            "crates/sdocx/src/ink.rs",
            "crates/sdocx/src/render.rs",
            "crates/sdocx/tests/fountain_v14_raster.rs",
        ]
    },
    "probe_sha256": hashlib.sha256(
        (root / "crates/sdocx/examples/fountain_v14_scale_probe.rs").read_bytes()
    ).hexdigest(),
}
print(json.dumps(receipt, indent=2))
Path(sys.argv[2]).write_text(json.dumps(receipt, indent=2) + "\n")
