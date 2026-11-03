"""Check retained export hashes and the recorded direct reader comparisons."""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pymupdf
import pypdfium2
import PIL
from PIL import Image, ImageChops

root = Path(sys.argv[1])
here = Path(__file__).resolve().parent
receipt = json.loads((here / "clean-reader-receipt.json").read_text())

versions = {
    "resvg": subprocess.check_output(["resvg", "--version"], text=True)
    .strip()
    .removeprefix("resvg "),
    "pymupdf": pymupdf.VersionBind,
    "pdfium": str(pypdfium2.PDFIUM_INFO),
    "pypdfium2": str(pypdfium2.PYPDFIUM_INFO.version),
    "pillow": PIL.__version__,
}
assert versions == receipt["reader_versions"], (versions, receipt["reader_versions"])


def stats(a, b):
    values = list(ImageChops.difference(a, b).getdata())
    return (
        sum(map(sum, values)),
        sum(any(pixel) for pixel in values),
        max(map(max, values)),
    )


with tempfile.TemporaryDirectory() as temporary:
    for case, mode, scale in [
        ("320", "one", 1),
        ("64-alternate", "two", 2),
        ("65-baseline", "two", 2),
        ("128-baseline", "two", 2),
    ]:
        paths = [root / ("sb-gradient-base-" + case), root / ("sb-gradient-" + case)]
        for reader in ("resvg", "mupdf", "pdfium"):
            images = []
            for directory in paths:
                if reader == "resvg":
                    target = Path(temporary) / (case + "-" + directory.name + ".png")
                    subprocess.run(
                        ["resvg", str(directory / f"{mode}.svg"), str(target)],
                        check=True,
                    )
                    images.append(Image.open(target).convert("RGBA"))
                elif reader == "mupdf":
                    document = pymupdf.open(directory / f"{mode}.pdf")
                    pixmap = document[0].get_pixmap(
                        matrix=pymupdf.Matrix(
                            4 * scale / 3, 0, 0, 4 * scale / 3, 0.25, -0.5
                        ),
                        alpha=False,
                    )
                    images.append(
                        Image.frombytes(
                            "RGB", (pixmap.width, pixmap.height), pixmap.samples
                        )
                    )
                else:
                    document = pypdfium2.PdfDocument(str(directory / f"{mode}.pdf"))
                    images.append(
                        document[0]
                        .render(scale=4 * scale / 3, rev_byteorder=True)
                        .to_pil()
                        .convert("RGB")
                    )
            result = stats(*images)
            print(case, reader, result)
            assert list(result) == receipt["comparison"][case][reader]
            key = {"resvg": "svg", "mupdf": "mupdf", "pdfium": "pdfium"}[reader]
