# Longer Montblanc 2× size controls

Twelve fresh synthetic Drawing/host Mesa captures exercise sizes 0.5, 1 and 2 on 12-, 15-, 18- and 20-point arc, crossing, skip and wave trajectories. The capture adapter receives the requested points and pressures but does not independently read them back. The `long_independent_native_stream` Rust test matches every captured callback lane bitwise. The public 2× admission now includes 12–20 points at sizes 0.5–2; intermediate sizes and arbitrary paths remain unverified.

Every candidate reduces union RGB absolute error versus the generic fallback across SVG/resvg and direct PDF/MuPDF and PDFium. All overcover native support. This does not establish saved-note, native PDF, device or lifecycle parity. Native capture dependencies are partially pinned, as in `../long-2x-sizes/README.md`.

Unpack `native.tar.gz` into a fresh directory and set `MB_CAPTURE_WORK` to that directory; set `MB_EXPORT_WORK` to a separate fresh empty directory. Run `PYTHONPATH=/tmp/marker2-pdfium-python .venv/bin/python tools/montblanc/long-2x-size-v192/compare.py rebuild-verify`. The comparator verifies streams, frames, source and output hashes, renderer versions and metrics. For fresh captures use `capture` with an empty `MB_EXPORT_WORK`. Temporary exports are reproducible and should be discarded. The retained streams feed the durable Rust test.
