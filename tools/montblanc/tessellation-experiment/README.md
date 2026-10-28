# Montblanc vector subdivision experiment

Run `.venv/bin/python tools/montblanc/tessellation-experiment/compare.py /tmp/montblanc-tessellation-results.json` from this checkout. It archives pinned revision `353d70034cc20706d81542d2e38bd5109a05321a`, checks the synthetic fixture and retained capture hashes, and varies only subdivision count and the patch budget in the temporary archive. It requires offline Cargo dependencies, resvg 0.45.1, Pillow 12.1.1, and PyMuPDF 1.28.2. Coordinate access to the shared agent Cargo target while it runs; it deletes generated SVG/PDF and intermediate images. No native execution is required.

The receipt contains support and RGB L1 metrics from the existing public-optin comparisons, sizes in bytes, and three warm wall-time runs for the complete six-case, three-mode probe (SVG and PDF with writes), not individual strokes. Build times and warm times depend on the machine and are not reproducibility assertions. Native frames are synthetic host Mesa captures, not device or saved-note parity. The RGB values use the existing premultiplied-alpha paper compositor. Fixed-function blend state remains unknown.

Sixteen subdivisions reduce overcoverage but can worsen RGB error versus eight, while SVG sizes approximately quadruple. Twelve often produces the lowest SVG error. Neither count is a general fidelity correction.
