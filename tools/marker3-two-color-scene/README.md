# Marker3 8.5 two-color destination control

A synthetic size-8.5 tool-1 Marker3 `5;` Drawing vector is rendered twice at 2× to the same native RGBA8 framebuffer. The first stroke uses the `phase2` input in `../marker3-terminal-size85`; the second reuses its native vector with a (+12,+8) output-pixel matrix translation, corresponding to (+6,+4) source coordinates. The two cases assign `0x8033cc55` and `0x80cc331a` to the first and second draws in opposite orders. The host directly overwrites the selected RT's color field between draws; the second stroke is not a separate Model object. Both draws composite to the same framebuffer, as recorded by the native GL call receipts, and reported GL errors are zero. The compact receipts omit blend-function values and matrix uniforms; those details require rerunning the capture script and inspecting its native trace. The native mask and RT lifecycle for independent strokes remains unresolved. This is a controlled host-Mesa experiment, not a serialized-note, device GPU, arbitrary-input, or independent-stroke claim.

`forward.rgba.zlib` and `reverse.rgba.zlib` retain the premultiplied native frames; JSON files retain hashes, draw destinations, color uniforms and GL checks. Run `.venv/bin/python tools/marker3-two-color-scene/capture.py` on the pinned host to regenerate and compare those frames, draws and uniforms. It calls the pre-existing native capture and has the local dependencies and same-vector bridge limitations documented in `../marker3-terminal-size85` and `../marker3-size9-expanded`.

`PYTHON=.venv/bin/python PYTHONPATH=/tmp/sdocx-marker3-v133-readers bash tools/marker3-two-color-scene/replay.sh` archives commit `9eaa8fc22d01d1564cc2a87c1b445e6147488460`, temporarily extends its size guard to 9, generates SVG and direct PDF from two source strokes, and checks exact metrics against retained native frames. It uses the retained lockfile, offline Cargo and shared agent target. The transparent native output is mathematically composited over #fcfcfc for PDF comparisons. Resvg 0.45.1, MuPDF 1.28.2 and pypdfium2 5.14.0 / PDFium 156.0.8076.0 are required.

| Color order | Mode | SVG RGBA L1 | MuPDF RGB L1 | PDFium RGB L1 | Missing native support pixels |
|---|---|---:|---:|---:|---:|
| green, red | baseline | 126971 | 247272 | 123681 | 0 |
| green, red | candidate | 108384 | 235583 | 107365 | 0 |
| red, green | baseline | 138344 | 228097 | 105222 | 0 |
| red, green | candidate | 122425 | 213104 | 85564 | 0 |

The large residuals and direct RT intervention prevent generalizing this experiment into a production gate. The comparison is a two-color assignment control on overlapping, translated copies; it is not a reversal of two independently acquired strokes.
