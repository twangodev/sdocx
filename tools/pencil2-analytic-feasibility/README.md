# Pencil2 source texture complexity

`python3 tools/pencil2-analytic-feasibility/texture_complexity.py` checks the
SHA-256 and dimensions of the packaged point and grain textures used by the
explicit host fragment model. It counts equal-value horizontal runs and vertical
neighbor changes, excluding the repeat boundary; the point count first maps the bytes to zero versus nonzero,
matching that model's point-atlas gate. The resulting counts are:

| Texture | Distinct modeled values | Horizontal runs | Vertical changes |
| --- | ---: | ---: | ---: |
| Grain, 150 × 150 | 229 | 22,299 | 22,171 |
| Point gate, 100 × 300 | 2 | 1,219 | 962 |

This measures the retained source texture, not a rendered stroke. In the host
model the nearest-sampled repeating grain period is 75 times the output scale;
therefore each grain texel spans half a framebuffer pixel at scale 1, equivalent
to half a source unit under that model’s scale convention. A pattern made
from exact constant regions could represent these discontinuities independently
of the output pixel grid, but would need to preserve the texture's coordinate
transform, clipping, multiplication by the per-stamp nonlinear alpha field,
and ordered source-over compositing. The run count is not a lower bound on
arbitrary vector encodings or a measurement of PDF size or reader performance.

The [radial alpha mask reader control](../pencil2-vector-width/README.md)
reports resvg, MuPDF and PDFium differences for a synthetic mask. This texture
count does not establish a scalable vector implementation, device behavior,
saved-page parity, or direct document-PDF behavior. No production dispatch
follows.
