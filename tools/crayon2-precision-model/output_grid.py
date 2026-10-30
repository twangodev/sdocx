"""Experimental fixed 192px source-texture grid; no framebuffer input."""

import numpy as np
import struct
import math
import sys
import hashlib
from pathlib import Path

f = np.float32
records = Path(sys.argv[1]).read_bytes()
if len(records) != 479 * 24:
    raise ValueError("expected source-derived independent stroke records")
tex = Path("tmp/vector-parity-plan/crayon2-native-next/native-uploads")
point = (tex / "g_Crayon2PointPtn.source.bin").read_bytes()
bg = (tex / "g_Crayon2BgPtn.source.bin").read_bytes()
assert (
    hashlib.sha256(point).hexdigest()
    == "673423c6fdae1ba344c452deefeb51b7b11365fead0bfbdd4b345eed4027f405"
)
assert (
    hashlib.sha256(bg).hexdigest()
    == "2df7013777862212381e424903db7b1194379db9c6fe6bb3dafa71a56573b2f3"
)
width = height = 192
mat = f(0.010416666977107525)
scale = f(1.7066665887832642)


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


def val(plane, x, y):
    c, dx, dy = plane
    return fm(dy, f(y), fm(dx, f(x), c))


svg = [
    '<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192" viewBox="0 0 192 192" shape-rendering="crispEdges"><rect width="192" height="192" fill="#4080c0"/>'
]
for x, y, rad, sel, bx, by in struct.iter_unpack("<6f", records):
    coords = []
    uv = []
    for sx, sy, pu, pv in [(-1, -1, 0, 1), (-1, 1, 0, 0), (1, -1, 1, 1), (1, 1, 1, 0)]:
        ps = []
        us = []
        for coord, off, sg in [(x, bx, sx), (y, by, sy)]:
            v = f(f(coord) + f(f(sg) * f(rad)))
            b = f(v + f(off))
            clip = f(f(mat * v) - f(1))
            bc = f(f(mat * b) - f(1))
            ps.append(f(f(f(clip + f(1)) * f(0.5)) * f(192)))
            us.append(f(f(bc + f(1)) * f(0.5)))
        coords.append(ps)
        uv.append(us + [f(pu), f(f(1 / 3) * f(f(sel) + f(pv)))])
    planes = []
    for ids in [(0, 1, 2), (2, 1, 3)]:
        planes.append(
            [setup([coords[i] for i in ids], [uv[i][a] for i in ids]) for a in range(4)]
        )
    x0, y0 = coords[0]
    x1 = coords[2][0]
    y1 = coords[1][1]
    for py in range(
        max(0, math.ceil(float(y0) - 0.5)), min(192, math.ceil(float(y1) - 0.5))
    ):
        for px in range(
            max(0, math.ceil(float(x0) - 0.5)), min(192, math.ceil(float(x1) - 0.5))
        ):
            a = (px + 0.5 - float(x0)) / float(x1 - x0)
            b = (py + 0.5 - float(y0)) / float(y1 - y0)
            p = planes[0 if a + b <= 1 else 1]
            ux, uy, pu, pv = [val(q, px, py) for q in p]
            tx = int(math.floor(float(f(ux * scale)) * 150)) % 150
            ty = int(math.floor(float(f(uy * scale)) * 150)) % 150
            ix = min(9, max(0, int(math.floor(float(pu) * 10))))
            iy = min(29, max(0, int(math.floor(float(pv) * 30))))
            alpha = min(
                1,
                float(
                    f(
                        f(
                            f(f(170 / 255) * f(point[iy * 10 + ix] / 255))
                            * f(bg[ty * 150 + tx] / 255)
                        )
                        * f(5)
                    )
                ),
            )
            if alpha:
                svg.append(
                    f'<rect x="{px}" y="{191 - py}" width="1" height="1" fill="#b84c29" fill-opacity="{alpha:.9f}"/>'
                )
svg.append("</svg>")
Path(sys.argv[2]).write_text("\n".join(svg))
