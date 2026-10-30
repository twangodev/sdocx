import json, struct, zlib, numpy as np
from pathlib import Path

f = np.float32
r = json.loads(Path("tools/crayon2-precision-model/receipt.json").read_text())
v = zlib.decompress(
    Path("tools/crayon2-independent-control/native-vector.z").read_bytes()
)
scores = {}
mism = {}


def decode(rgb, expected):
    low = int.from_bytes(bytes(rgb[:3]), "big")
    return min(
        [
            struct.unpack("<f", struct.pack("<I", high << 24 | low))[0]
            for high in range(0x3C, 0x41)
        ],
        key=lambda a: abs(a - expected),
    )


def fm(a, b, c):
    return f(float(a) * float(b) + float(c))


for k, entry in r["stamps"].items():
    x, y, rad, _, bx, by = struct.unpack_from("<6f", v, 24 * int(k))
    mat = f(0.010416666977107525)
    for row in entry["observations"]:
        px, py = row["xy"]
        for center, offset, p, mode in [
            (x, bx, px + 0.5, "xuv"),
            (y, by, 191 - py + 0.5, "yuv"),
        ]:
            vertices = []
            for sign in [-1, 1]:
                coord = f(f(center) + f(f(sign) * f(rad)))
                bg = f(coord + f(offset))
                clip = f(f(mat * coord) - f(1))
                bgclip = f(f(mat * bg) - f(1))
                vertices.append(
                    (
                        f(f(f(clip + f(1)) * f(0.5)) * f(192)),
                        f(f(bgclip + f(1)) * f(0.5)),
                    )
                )
            (s0, u0), (s1, u1) = vertices
            p = f(p)
            slope = f(f(u1 - u0) / f(s1 - s0))
            d = f(p - s0)
            actual = decode(row["encoded_rgba"][mode], (float(p) + offset) / 192)
            vals = {
                "base": f(u0 + f(d * slope)),
                "fma": fm(d, slope, u0),
                "origin_fma": fm(p, slope, f(u0 - f(s0 * slope))),
                "origin_fma2": fm(p, slope, fm(-s0, slope, u0)),
                "double": f(
                    float(u0)
                    + (float(p) - float(s0))
                    * (float(u1) - float(u0))
                    / (float(s1) - float(s0))
                ),
                "right": f(u1 + f(f(p - s1) * slope)),
                "right_fma": fm(f(p - s1), slope, u1),
            }
            for name, val in vals.items():
                scores[name] = scores.get(name, 0) + (float(val) == actual)
                if float(val) != actual:
                    mism.setdefault(name, []).append(
                        (k, px, py, mode, (float(val) - actual) * 2**24)
                    )
print(scores)
for k in scores:
    print(k, mism.get(k, []))
