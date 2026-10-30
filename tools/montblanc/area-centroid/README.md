# Montblanc area-centroid control

This rejected candidate samples clipped polygons at their area-weighted centroid in source XY, interpolating all five vertex attributes; degenerate polygons retain the vertex average. The baseline is the renderer at `6d8d0300`. On 12 retained 1x synthetic host-Mesa Drawing controls, candidate/baseline union RGB L1 error ratios are 1.0235–1.0587 (resvg), 1.0433–1.0827 (MuPDF), and 1.0425–1.0919 (PDFium). On four independent 2x controls they are 0.9996–1.0319, 1.0008–1.0553, and 0.9996–1.0572, respectively. No device or saved-note parity is implied. Each metric vector is native support, output support, intersection, union, and union RGB absolute error. The receipts retain capture and stream identities, selected source and build hashes, reader versions, and output hashes; these are not a complete build or native dependency closure.

Reproduce using fresh directories and the repository's Python environment, resvg, and PDFium installation. `rebuild-verify` compares the complete receipt including script/source hashes and output hashes, so versions and environment must match. The archive includes the retained native capture archives and input streams.

```sh
mkdir -p /tmp/mb-centroid-replay/source /tmp/mb-centroid-replay/capture-unit /tmp/mb-centroid-replay/capture-double /tmp/mb-centroid-replay/unit /tmp/mb-centroid-replay/double
git archive 6d8d0300 | tar -x -C /tmp/mb-centroid-replay/source
cp -r tools/montblanc/area-centroid /tmp/mb-centroid-replay/source/tools/montblanc/
tar -xf tools/montblanc/long-unit-size-v205/native.tar.gz -C /tmp/mb-centroid-replay/capture-unit
tar -xf tools/montblanc/long-2x-independent/native.tar.gz -C /tmp/mb-centroid-replay/capture-double
MB_CAPTURE_WORK=/tmp/mb-centroid-replay/capture-unit MB_EXPORT_WORK=/tmp/mb-centroid-replay/unit PYTHONPATH=/tmp/marker2-pdfium-python CARGO_BUILD_JOBS=2 .venv/bin/python /tmp/mb-centroid-replay/source/tools/montblanc/area-centroid/compare-unit.py rebuild-verify
MB_CAPTURE_WORK=/tmp/mb-centroid-replay/capture-double MB_EXPORT_WORK=/tmp/mb-centroid-replay/double PYTHONPATH=/tmp/marker2-pdfium-python CARGO_BUILD_JOBS=2 .venv/bin/python /tmp/mb-centroid-replay/source/tools/montblanc/area-centroid/compare-double.py rebuild-verify
```

The scripts build isolated baseline and candidate workspaces and apply the pinned patch only to the candidate. They use the shared agent Cargo target and resource settings. `rebuild-record` writes a fresh receipt into the export directory instead of comparing with the retained one.
