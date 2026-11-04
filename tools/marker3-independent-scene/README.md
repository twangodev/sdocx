# Marker3 8.5 independent synthetic strokes

Two separately constructed Model ObjectStroke objects enter two independently constructed ObjectDrawing instances sharing one explicit host context. Native factory selection creates distinct Marker3 V5 drawables and RTs. Their native callback vectors are each explicitly reattached to the corresponding cleared RT+88 slot, then native RT Update and Draw render to separate mask framebuffers and the same 256×256 RGBA8 destination at 2×. This is a synthetic host-Mesa experiment with explicit vector bridges, not evidence of native automatic attachment, serialized notes, or device rendering. The retained prototype inherits a report schema written for one stroke; only the selected facts in `receipt.json` describe this experiment. It does not establish saved-redraw vector equality for the second stroke.

The first is translucent green; the second is translucent red, with independently submitted points translated by (+6,+4). Reversing their order keeps colors attached to their own strokes. Both native vectors contain 37 XY pairs and differ from an exact translation by at most 0.00000190735 in a coordinate. The receipt retains exact input, vectors, relevant library/helper hashes, forward draw and color traces, GL error checks, frame hashes and export metrics. The source capture template checks the pinned dependencies listed in `../marker3-zero/hashes.json`; the other host and reader dependencies remain those described in `../marker3-terminal/README.md`.

`PYTHON=.venv/bin/python tools/marker3-independent-scene/replay-native.sh` uses an isolated temporary directory, regenerates both frames, and checks every byte against the compressed frames. It also checks the first Model getter inputs, the submitted second inputs and color getter, both complete vectors, recorded source and library hashes, forward draw and color traces, reverse draw topology and color order, and zero GL errors. The full reverse trace is not retained; replay checks its mask/destination relationships and uniforms. The harness constructs two Drawing instances and bridges each captured vector; it does not model a full native page lifecycle. SVG and direct PDF comparisons were made against an archived `9eaa8fc22d01d1564cc2a87c1b445e6147488460` crate with the size guard temporarily extended to 9, using the phase2 input and renderers described in `../marker3-two-color-scene/README.md`. PDF compares against the native frame composited over #fcfcfc.

| Stroke order | SVG L1 baseline / candidate | MuPDF L1 baseline / candidate | PDFium L1 baseline / candidate | Missing native support pixels |
|---|---:|---:|---:|---:|
| green then red | 57373 / 20334 | 178624 / 148453 | 58933 / 23365 | 0 |
| red then green | 58448 / 20500 | 176931 / 148112 | 57070 / 22791 | 0 |

The historical candidate improved these bounded controls, with residual error.
The exporter, comparator and export-replay script were retired. Native replay,
its verifier, capture patches, frames and receipt remain. These reader metrics
are historical; maintained Rust Marker3 tests do not directly consume these
two scene fixtures. This evidence alone does not justify wider admission.
