# Direct document radial alpha control

A constructed synthetic type-8 line (not a saved input) uses a radial gradient with saved black ARGB stops
`ff000000` at 0.25 and `00000000` at 0.75, centered at (0.5, 0.5).
The line spans a square reference box at (40, 40) with width and height 2 or
64; its stroke width equals the box size. The current SVG projection produces
a transparent center and an opaque exterior with a hard stop at 0.25. It is not
the smooth radial mask in `tools/pencil2-vector-width/radial_mask_control.py`.
Both exports use the same document; PDF uses 72 dpi and a white background.
This is a synthetic source control, with no native pixel comparison.

On resvg 0.45.1, MuPDF 1.28.2 and PDFium 156.0.8076.0, the absolute red-channel
error sums versus resvg were 255/255 at size 2 and 6885/8415 at size 64,
respectively; the maximum was 255 in every case. At size 2 the red samples
were resvg `[[0,0],[0,0]]` and both PDF readers `[[0,0],[0,255]]`.
Each PDF had two radial shadings, one soft-mask reference and no image objects;
these counts do not verify resource connectivity. The hard edge and reader
sampling confound attribution, and these results do not establish a correction.
Current repository source and renderer dependencies are mutable.

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path tools/direct-radial-shape/Cargo.toml
PYTHONPATH=/tmp/sdocx-marker3-v133-readers .venv/bin/python tools/direct-radial-shape/compare.py
```

The comparison needs resvg on PATH and Pillow, PyMuPDF and pypdfium2 in the Python environment. The shown `.venv` and `PYTHONPATH` are local environment paths; use equivalents on another host. Set `SDOCX_RADIAL_CONTROL_RUNNER` to override the default shared-target runner path. The runner creates new output files exclusively and refuses to overwrite existing files. The comparison validates the generated SVG gradient, stop values, and line geometry before rendering. It uses a temporary
directory and prints results rather than asserting version-sensitive pixels.
