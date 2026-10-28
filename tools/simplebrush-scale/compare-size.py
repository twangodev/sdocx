import hashlib
import pymupdf
import json
import subprocess
import re
import zlib
import struct
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
b = Path(sys.argv[1]).resolve()
receipt = json.loads((root / "conformance/simplebrush-scale-2x-size-host.json").read_text())
case = next(c for c in receipt["cases"] if c["case"] == sys.argv[2])
x0, y0, x1, y1 = case["alpha_bounds"]
crop = (root / "conformance" / case["crop"]["file"]).read_bytes()
assert hashlib.sha256(crop).hexdigest() == case["crop"]["sha256"]
assert len(crop) == (x1 - x0) * (y1 - y0) * 4
native = bytearray(256 * 256 * 4)
for y in range(y0, y1):
    native[(y * 256 + x0) * 4:(y * 256 + x1) * 4] = crop[
        (y - y0) * (x1 - x0) * 4:(y - y0 + 1) * (x1 - x0) * 4
    ]
assert hashlib.sha256(native).hexdigest() == case["frame_sha256"]
print("bounds", x0, y0, x1, y1, "pixels", case["nonzero_alpha"], "frame", case["frame_sha256"])
assert subprocess.check_output(["resvg", "--version"], text=True).strip() == "0.45.1"
assert pymupdf.VersionBind == "1.28.2" and pymupdf.VersionFitz == "1.28.2"
paper_doc = pymupdf.open(b / "paper.pdf")
paper = paper_doc[0].get_pixmap(
    matrix=pymupdf.Matrix(8 / 3, 0, 0, 8 / 3, 0.25, -0.5), alpha=False
)
for mode in ("generic", "one", "two"):
    s = (b / f"{mode}.svg").read_text()
    body = s[s.index(">") + 1 : s.rindex("</svg>")]
    body = re.sub(r'<rect fill="#fcfcfc"[^>]*/>', "", body, count=1)
    s = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><g transform="matrix(2 0 0 -2 .25 512.5)">'
        + body
        + "</g></svg>"
    )
    (b / "compare.svg").write_text(s)
    subprocess.run(
        ["resvg", str(b / "compare.svg"), str(b / "compare.png")], check=True
    )
    png = (b / "compare.png").read_bytes()
    pos = 8
    compressed = b""
    while pos < len(png):
        n = int.from_bytes(png[pos : pos + 4], "big")
        t = png[pos + 4 : pos + 8]
        v = png[pos + 8 : pos + 8 + n]
        pos += n + 12
        if t == b"IHDR":
            assert struct.unpack(">IIBBBBB", v) == (256, 256, 8, 6, 0, 0, 0)
        if t == b"IDAT":
            compressed += v
        if t == b"IEND":
            break
    raw = zlib.decompress(compressed)
    prev = bytearray(1024)
    data = bytearray()

    def paeth(a, b, c):
        p = a + b - c
        d = [abs(p - a), abs(p - b), abs(p - c)]
        return [a, b, c][d.index(min(d))]

    for y in range(256):
        f = raw[y * 1025]
        row = bytearray(raw[y * 1025 + 1 : y * 1025 + 1025])
        for x in range(1024):
            a = row[x - 4] if x >= 4 else 0
            q = prev[x]
            c = prev[x - 4] if x >= 4 else 0
            row[x] = (
                row[x]
                + (
                    0
                    if f == 0
                    else a
                    if f == 1
                    else q
                    if f == 2
                    else (a + q) // 2
                    if f == 3
                    else paeth(a, q, c)
                )
            ) % 256
        data.extend(row)
        prev = row
    err = sum(abs(native[k] - data[k]) for k in range(3, len(native), 4))
    inter = sum(bool(native[k] and data[k]) for k in range(3, len(native), 4))
    union = sum(bool(native[k] or data[k]) for k in range(3, len(native), 4))
    doc = pymupdf.open(b / f"{mode}.pdf")
    pix = doc[0].get_pixmap(
        matrix=pymupdf.Matrix(8 / 3, 0, 0, 8 / 3, 0.25, -0.5), alpha=False
    )
    assert (pix.width, pix.height, pix.x, pix.y, pix.n) == (513, 513, 0, -1, 3)
    assert (paper.width, paper.height, paper.x, paper.y, paper.n) == (
        513,
        513,
        0,
        -1,
        3,
    )
    r = pix.samples
    background = paper.samples
    rgb = 0
    rgb_local = 0
    matched = 0
    matched_local = 0
    for y in range(256):
        for x in range(256):
            i = (y * 256 + x) * 4
            j = ((511 - y - pix.y) * pix.width + x - pix.x) * 3
            a = native[i + 3]
            for k in range(3):
                difference = abs(
                    r[j + k] - (native[i + k] + (252 * (255 - a) + 127) // 255)
                )
                rgb += difference
                matched_difference = abs(
                    r[j + k]
                    - (native[i + k] + (background[j + k] * (255 - a) + 127) // 255)
                )
                matched += matched_difference
                if x0 - 2 <= x < x1 + 2 and y0 - 2 <= y < y1 + 2:
                    rgb_local += difference
                    matched_local += matched_difference
    index = ("generic", "one", "two").index(mode)
    assert err == case["svg_alpha_l1_generic_one_two"][index]
    assert matched == case["matched_paper_pdf_rgb_l1_generic_one_two"][index]
    print(
        mode,
        {
            "svg_support": [inter, union],
            "svg_alpha_error": err,
            "pdf_rgb_error": rgb,
            "pdf_rgb_error_local": rgb_local,
            "matched": matched,
            "matched_local": matched_local,
        },
    )
