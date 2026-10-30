# Montblanc polygon cutoff

The 32-sided inscribed cutoff is emitted as one convex polygon per subdivision patch, sampled at its vertex-average UV. It remains an approximation: the polygon omits a thin band inside the native circular cutoff, and native smooth antialiasing differs from geometric clipping. Two reused scratch buffers bound clipping allocations; the existing 32,768-subdivision budget also bounds output polygons, each at most 35 vertices.

The receipts compare the final renderer source in an isolated archive against the original subdivision renderer and retained synthetic host-Mesa Drawing frames. They cover twelve 1x long-path controls and four 2x controls. Union RGB L1 error ratios against the baseline are 0.940–0.984 (resvg), 0.912–0.933 (MuPDF), and 0.905–0.934 (PDFium) at 1x, and 0.863–0.900, 0.799–0.864, and 0.793–0.867 at 2x. All controls improved in all three readers. SVG grows 1.274–1.277x and PDF 1.402–1.642x. Four additional three-point controls improve resvg and MuPDF error; device, saved-note, and arbitrary-transform parity remain unverified.

To replay from the repository root, choose new empty directories and run:

```sh
mkdir -p /tmp/mb-replay/source /tmp/mb-replay/capture-unit /tmp/mb-replay/capture-double /tmp/mb-replay/out-unit /tmp/mb-replay/out-double
git archive a03df32d0eea4ea119622c1f02fd2e0ffe8ad81b | tar -x -C /tmp/mb-replay/source
cp -r tools/montblanc/polygon-cutoff /tmp/mb-replay/source/tools/montblanc/
cp /tmp/mb-replay/source/crates/sdocx/src/render/montblanc_candidate.rs /tmp/mbclip-original.rs
patch -d /tmp/mb-replay/source -p1 < tools/montblanc/polygon-cutoff/candidate.patch
tar -xf tools/montblanc/long-unit-size-v205/native.tar.gz -C /tmp/mb-replay/capture-unit
tar -xf tools/montblanc/long-2x-independent/native.tar.gz -C /tmp/mb-replay/capture-double
MB_CAPTURE_WORK=/tmp/mb-replay/capture-unit MB_EXPORT_WORK=/tmp/mb-replay/out-unit PYTHONPATH=/tmp/marker2-pdfium-python CARGO_BUILD_JOBS=2 .venv/bin/python /tmp/mb-replay/source/tools/montblanc/polygon-cutoff/compare-unit.py rebuild-record
MB_CAPTURE_WORK=/tmp/mb-replay/capture-double MB_EXPORT_WORK=/tmp/mb-replay/out-double PYTHONPATH=/tmp/marker2-pdfium-python CARGO_BUILD_JOBS=2 .venv/bin/python /tmp/mb-replay/source/tools/montblanc/polygon-cutoff/compare-double.py rebuild-record
```

The scripts pin the supplied patch, patched renderer and original renderer, verify the captured input streams, and record selected build inputs, reader versions and output hashes. These pins are not a full build closure. The commands use the project Python environment, resvg, and the PDFium Python installation at the indicated path; the scripts set the shared Cargo resource settings. Existing `/tmp/mbclip-original.rs` is overwritten by the commands. `rebuild-record` writes to the fresh output directories; it does not overwrite these retained receipts.
