import base64
import io
import json
import subprocess
import tempfile
import zlib
from pathlib import Path

import numpy as np
import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[2] / "conformance/brushpen-second-native"
WIDTH = 321
HEIGHT = 228
FRAME_NAMES = (
    "frame",
    "matrix-frame",
    "scale-frame",
    "nonuniform-frame",
    "shear-frame",
    "rotate-visible-frame",
    "flip-visible-frame",
)


def load_frame(name):
    filename = "frame-rgba8.zlib" if name == "frame" else f"{name}.rgba.zlib"
    raw = zlib.decompress((ROOT / filename).read_bytes())
    return np.frombuffer(raw, dtype=np.uint8).reshape(HEIGHT, WIDTH, 4).copy()


def encode_png(frame):
    # Native readback is premultiplied and bottom-up; PNG stores straight alpha.
    straight = np.flipud(frame).copy()
    alpha = straight[:, :, 3].astype(np.uint32)
    rgb = straight[:, :, :3].astype(np.uint32)
    divisor = np.maximum(alpha[:, :, None], 1)
    straight[:, :, :3] = np.where(
        alpha[:, :, None] > 0,
        np.minimum(255, (rgb * 255 + divisor // 2) // divisor),
        0,
    ).astype(np.uint8)
    buffer = io.BytesIO()
    Image.fromarray(straight).save(buffer, format="PNG")
    return buffer.getvalue()


def render_svg(png, directory):
    encoded = base64.b64encode(png).decode()
    svg_path = directory / "frame.svg"
    image_path = directory / "frame.png"
    svg_path.write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}">'
        f'<image width="{WIDTH}" height="{HEIGHT}" '
        f'href="data:image/png;base64,{encoded}"/></svg>'
    )
    subprocess.run(
        ["resvg", str(svg_path), str(image_path)],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    pixels = np.array(Image.open(image_path).convert("RGBA")).astype(np.uint32)
    pixels[:, :, :3] = (pixels[:, :, :3] * pixels[:, :, 3, None] + 127) // 255
    return pixels


def render_pdf(png):
    document = pymupdf.open()
    page = document.new_page(width=WIDTH, height=HEIGHT)
    page.insert_image(page.rect, stream=png)
    pixmap = page.get_pixmap(alpha=True)
    return np.frombuffer(pixmap.samples, np.uint8).reshape(HEIGHT, WIDTH, 4)


def error_metrics(actual, reference):
    difference = np.abs(actual.astype(np.int32) - reference)
    return {
        "rgba_l1": int(difference.sum()),
        "alpha_l1": int(difference[:, :, 3].sum()),
    }


def main():
    results = {}
    with tempfile.TemporaryDirectory() as temporary_directory:
        directory = Path(temporary_directory)
        for name in FRAME_NAMES:
            frame = load_frame(name)
            png = encode_png(frame)
            reference = np.flipud(frame).astype(np.int32)
            results[name] = {
                "svg_resvg": error_metrics(render_svg(png, directory), reference),
                "pdf_mupdf": error_metrics(render_pdf(png), reference),
            }
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
