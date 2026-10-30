import json, struct, zlib, numpy as np, itertools
from pathlib import Path

f = np.float32
r = json.loads(
    Path(
        __import__("sys").argv[1]
        if len(__import__("sys").argv) > 1
        else "tools/crayon2-precision-model/receipt.json"
    ).read_text()
)
v = zlib.decompress(
    Path("tools/crayon2-independent-control/native-vector.z").read_bytes()
)
mat = f(0.010416666977107525)


def dec(rgb, e):
    lo = int.from_bytes(bytes(rgb[:3]), "big")
    return min(
        (
            struct.unpack("<f", struct.pack("<I", h << 24 | lo))[0]
            for h in range(0x3C, 0x41)
        ),
        key=lambda v: abs(v - e),
    )


def fm(a, b, c):
    return f(float(a) * float(b) + float(c))


def setup(p, a):
    (x0, y0), (x1, y1), (x2, y2) = p
    a0, a1, a2 = a
    dx01 = f(x0 - x1)
    dy01 = f(y0 - y1)
    dx20 = f(x2 - x0)
    dy20 = f(y2 - y0)
    inv = f(f(1) / f(f(dx01 * dy20) - f(dy01 * dx20)))
    da01 = f(a0 - a1)
    da20 = f(a2 - a0)
    dx = f(f(da01 * f(dy20 * inv)) - f(da20 * f(dy01 * inv)))
    dy = f(f(da20 * f(dx01 * inv)) - f(da01 * f(dx20 * inv)))
    c = f(a0 - f(f(dx * f(x0 - f(0.5))) + f(dy * f(y0 - f(0.5)))))
    return c, dx, dy


scores = {}
errs = {}
for k, e in r["stamps"].items():
    x, y, rad, _, bx, by = struct.unpack_from("<6f", v, int(k) * 24)
    coords = []
    uvs = []
    for sx, sy in [(-1, -1), (-1, 1), (1, -1), (1, 1)]:
        ps = []
        us = []
        for c, off, sg in [(x, bx, sx), (y, by, sy)]:
            coord = f(f(c) + f(f(sg) * f(rad)))
            bg = f(coord + f(off))
            clip = f(f(mat * coord) - f(1))
            bgclip = f(f(mat * bg) - f(1))
            ps.append(f(f(f(clip + f(1)) * f(0.5)) * f(192)))
            us.append(f(f(bgclip + f(1)) * f(0.5)))
        coords.append(ps)
        uvs.append(us)
    for row in e["observations"]:
        px, py = row["xy"]
        py = 191 - py
        # lower-left triangle or upper-right in window coordinates
        ax = (px + 0.5 - float(coords[0][0])) / (
            float(coords[2][0]) - float(coords[0][0])
        )
        ay = (py + 0.5 - float(coords[0][1])) / (
            float(coords[1][1]) - float(coords[0][1])
        )
        tri = (0, 1, 2) if ax + ay <= 1 else (2, 1, 3)
        for perm in itertools.permutations(range(3)):
            inds = [tri[i] for i in perm]
            for axis, mode, off, p in [(0, "xuv", bx, px), (1, "yuv", by, py)]:
                actual = dec(row["encoded_rgba"][mode], (p + 0.5 + off) / 192)
                c, dx, dy = setup(
                    [coords[i] for i in inds], [uvs[i][axis] for i in inds]
                )
                for m, val in [
                    ("add", f(f(c + f(dx * f(px))) + f(dy * f(py)))),
                    ("fma", fm(dy, f(py), fm(dx, f(px), c))),
                ]:
                    key = (perm, m)
                    scores[key] = scores.get(key, 0) + (float(val) == actual)
                    if val != actual:
                        errs.setdefault(key, []).append(
                            (k, row["xy"], mode, (float(val) - actual) * 2**24)
                        )
for k in sorted(scores, key=scores.get, reverse=True):
    print(k, scores[k], errs.get(k, []))
