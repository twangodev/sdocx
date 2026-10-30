"""Compare both admitted widths against retained independent native lifecycles."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib

import numpy as np
from PIL import Image
import pymupdf
import pypdfium2


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exporter", type=Path)
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    exporter = args.exporter.resolve()
    root = Path(__file__).resolve().parents[2]
    evidence = root / "tools/marker2-scale-investigation"
    cases = {
        "width2": (
            evidence / "expanded/independent-lifecycle",
            evidence / "v61/bend/w2-translated-a255.json",
        ),
        "width3": (
            evidence / "width3-controls",
            evidence / "fringe-sweep/bend/w3-translated-a255.json",
        ),
    }
    result = {
        "scope": "Two synthetic independent native Drawing/RTV lifecycles per width; host Mesa, not Android device or saved-page parity.",
        "exporter_sha256": sha(exporter),
        "exporter_source_sha256": sha(Path(__file__).with_name("export.rs")),
        "comparator_sha256": sha(Path(__file__)),
        "cases": {},
    }
    for name, (base, source_path) in cases.items():
        receipt = json.loads((base / "receipt.json").read_text())
        assert not receipt["nonzero_gl_errors"]
        for field in ("object_strokes", "drawings", "pens", "renderers"):
            assert receipt[f"independent_{field}"]
        assert receipt["native_vector_sha256"] == receipt["second_native_vector_sha256"]
        for frame, key in (("first", "first_sha256"), ("second", "second_sha256")):
            raw = zlib.decompress((base / f"{frame}.rgba.zlib").read_bytes())
            assert len(raw) == 256 * 256 * 4
            assert hashlib.sha256(raw).hexdigest() == receipt[key]
        native = np.frombuffer(raw, dtype=np.uint8).reshape(256, 256, 4)[::-1].astype(np.int32)
        source = json.loads(source_path.read_text())
        assert source["source"]["source_callback_vector_sha256"] == receipt["native_vector_sha256"]
        assert source["native_input"]["translation_f32"] == receipt["matrices"][0][12:14]
        inputs = source["native_input"]["source_case"]
        inputs["color_argb"] = "0x80994422"
        dx, dy = source["native_input"]["translation_f32"]

        def decode(value):
            return struct.unpack("<f", bytes.fromhex(value))[0]

        def encode(value):
            return struct.pack("<f", value).hex()

        inputs["points_f32_bits"] = [
            [encode(decode(x) + dx / 2), encode(128 - dy / 2 - decode(y))]
            for x, y in inputs["points_f32_bits"]
        ]
        control = {
            "source_sha256": sha(source_path),
            "native_receipt_sha256": sha(base / "receipt.json"),
            "native_frame_sha256": sha(base / "second.rgba.zlib"),
        }
        with tempfile.TemporaryDirectory(prefix="sdocx-marker2-") as temporary:
            output = Path(temporary)
            (output / "input.json").write_text(json.dumps({"cases": {"test": {"input": inputs}}}))
            for mode in ("baseline", "candidate"):
                env = dict(os.environ, MARKER2_CANDIDATE="2" if mode == "candidate" else "")
                subprocess.run([exporter, output], env=env, check=True, capture_output=True)
                svg, pdf, png = (output / file for file in ("case-1.svg", "case-1-direct.pdf", "case-1.png"))
                subprocess.run(["resvg", "--zoom", "2", svg, png], check=True, capture_output=True)
                metrics = {"svg_sha256": sha(svg), "pdf_sha256": sha(pdf)}
                for reader in ("resvg", "mupdf", "pdfium"):
                    if reader == "resvg":
                        with Image.open(png) as image:
                            pixels = np.asarray(image.convert("RGB"), dtype=np.int32)[:256, :256]
                    elif reader == "mupdf":
                        with pymupdf.open(pdf) as document:
                            pix = document[0].get_pixmap(matrix=pymupdf.Matrix(8 / 3, 8 / 3), colorspace=pymupdf.csRGB, alpha=False)
                            pixels = np.frombuffer(pix.samples, dtype=np.uint8).reshape(pix.height, pix.width, 3).astype(np.int32)[:256, :256]
                    else:
                        with pypdfium2.PdfDocument(pdf) as document:
                            pixels = np.asarray(document[0].render(scale=8 / 3).to_pil().convert("RGB"), dtype=np.int32)[:256, :256]
                    background = [251, 252, 251] if reader == "mupdf" else [252, 252, 252]
                    assert pixels.shape == (256, 256, 3) and pixels[0, 0].tolist() == background
                    expected = native[:, :, :3] + np.rint(pixels[0, 0] * (255 - native[:, :, 3, None]) / 255).astype(np.int32)
                    metrics[f"{reader}_rgb_absolute_error_sum"] = int(np.abs(pixels - expected).sum())
                control[mode] = metrics
        for reader in ("resvg", "mupdf", "pdfium"):
            key = f"{reader}_rgb_absolute_error_sum"
            assert control["candidate"][key] < control["baseline"][key], (name, reader, control)
        result["cases"][name] = control
    formatted = json.dumps(result, indent=2) + "\n"
    if args.receipt:
        args.receipt.write_text(formatted)
    print(formatted, end="")


if __name__ == "__main__":
    main()
