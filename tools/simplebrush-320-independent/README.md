# Independent 320-point SimpleBrush control

This tool-2 size-8 host-Mesa control uses the existing Rust fixture's alternate path and five-value pressure cycle. All points lie inside the 256×256 frame, with room for the brush footprint. The 320-point input, unlike the earlier reversed control, uses a combined change of trajectory and pressures; this does not isolate their individual effects. The native geometry matches the actual Rust sampler byte for byte. The opt-in one-pixel profile reduces SVG alpha error from 690,907 to 10,118, MuPDF matched-paper RGB error from 765,930 to 65,504, and PDFium RGB error from 763,386 to 110,461. PDFium uses an unshifted grid relative to the native fractional matrix. Profile SVG/PDF sizes are 465,270/2,235,640 bytes versus 36,528/4,027 bytes for fallback. The two-pixel profile remains fallback. These are synthetic host comparisons, not device or saved-page parity.

The compressed frame and geometry and compact receipt retain the native comparison. Capture depends on the pinned project-local scratch inputs checked by the underlying capture script. To repeat the native capture and public export comparison in an empty directory:

```sh
mkdir -p /tmp/NEW
python3 tools/simplebrush-320-independent/capture.py /tmp/NEW/native.json
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 SDOCX_POINTS_COUNT=320 PROBE_SIZE=8 VARIANT=alternate SDOCX_V62_NATIVE=/tmp/NEW/native.json SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/NEW cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-one-long/compare.py /tmp/NEW
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/simplebrush-one-long/pdfium.py /tmp/NEW
```

Comparators require resvg 0.45.1, PyMuPDF 1.28.2, and pypdfium2 5.9.0 with PDFium 150.0.7869.0. Native host coverage is bounded to this control; other paths remain extrapolations.

Run `PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/simplebrush-320-independent/verify.py` to replay the retained frame and geometry against a clean archive of revision `1328cff5`, verify source and export hashes, and check the three readers and fallback. The replay does not rerun native execution. The listed hashes do not pin the full build or transitive capture environment.
