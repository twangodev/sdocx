import json, struct, numpy as np, itertools, ctypes, hashlib, sys
libm = ctypes.CDLL("libm.so.6")
libm.fmaf.argtypes = [ctypes.c_float] * 3
libm.fmaf.restype = ctypes.c_float
from pathlib import Path

f = np.float32
fused_vertex = '--fused-vertex' in sys.argv
fixture = Path('tools/crayon2-f32-interpolation/translated-uv-receipt.json')
assert hashlib.sha256(fixture.read_bytes()).hexdigest() == '993df346c30a506a6be3b739809deb7ff41b681714482ba9e650898f08170c10'
vector = Path('tools/crayon2-document-holdout/vector.bin')
assert hashlib.sha256(vector.read_bytes()).hexdigest() == '15509eda1113e0bfd007455ba4767b1cba2a22391487f94c75e4121d98ac5f8f'
r = json.loads(fixture.read_text())
v = vector.read_bytes()
mat = f(0.012500000186264515)


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
    return f(libm.fmaf(float(a), float(b), float(c)))


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
        for c, off, sg, trans in [(x, bx, sx, f(-.8375)), (y, by, sy, f(-.8875))]:
            coord = f(f(c) + f(f(sg) * f(rad)))
            bg = f(coord + f(off))
            clip = fm(mat, coord, trans) if fused_vertex else f(f(mat * coord) + trans)
            bgclip = fm(mat, bg, trans) if fused_vertex else f(f(mat * bg) + trans)
            ps.append(f(f(f(clip + f(1)) * f(0.5)) * f(160)))
            us.append(f(f(bgclip + f(1)) * f(0.5)))
        coords.append(ps)
        uvs.append(us)
    for row in e["observations"]:
        px, py = row["xy"]
        py = 159 - py
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
                actual = dec(row["encoded_rgba"][mode], (p + 0.5 + off) / 160)
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
key = ((0, 1, 2), 'fma')
expected = 17 if fused_vertex else 21
assert sum(len(e['observations']) for e in r['stamps'].values()) * 2 == 22
assert scores[key] == expected, (scores[key], errs.get(key))
print(f"{'fused vertex' if fused_vertex else 'baseline'}: {scores[key]}/22 axis observations; mismatches {errs.get(key, [])}")
