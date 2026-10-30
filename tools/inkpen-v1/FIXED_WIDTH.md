# InkPen V1 fixed-width host controls

The native fixtures contain synthetic ObjectStroke setter inputs run through native Drawing, InkPen V1 and host Mesa. They are not device or saved-note evidence. The two paint matrices cover widths 0, 0.5, 3, 6, 12, 30, 50, 100 and 200, tools 0–4 and both curve modes at width 6, with one bent 2× and one fractional-phase 3× control, and one straight 4× control. The event-boundary matrix independently records both Drawing and saved-redraw event prefixes and terminal ranges.

Reproduce the captures with the project-local native runtime and pinned helpers already required by `tools/inkpen-capture/inkpen_v1_scale_capture.py`:

```sh
python3 tools/inkpen-v1/fixed_width_matrix.py /tmp/v1-fixed-native-new.json
python3 tools/inkpen-v1/fixed_width_extended.py /tmp/v1-fixed-extended-new.json
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias python3 tools/inkpen-v1/fixed_width_boundaries.py /tmp/v1-fixed-boundaries-new.json
```

These commands create new output files and refuse existing paths. They use native ARM64 libraries in the project-local capture environment and a synthetic Mesa surface. The paint wrapper uses a private temporary directory for readback intermediates. The event harness's curved `summary.sample_ends` omits the initial zero; compare its individual event returns to the full Rust cutoff sequence, as the probe does.

The pinned helper checkout at `tmp/conformance-cleanup/native-capture-a241793` is required by the current capture scripts; it is not tracked. Its `conformance/inkpen_joined_paint_native.py` and `inkpen_saved_caller_native.py` are checked against SHA-256 pins by `fixed_width_capture.py`; the boundary capture also uses its boundary and reuse harnesses. Recapture therefore requires that checkout and the project-local native runtime. The retained JSON fixtures are sufficient for the Rust conformance tests without recapture.

The Rust tests verify full vector words, replay boundaries, and invalid-width fallback. To regenerate the public-export comparison, create an empty directory and run the PDF-feature test with `SDOCX_V1_FIXED_EXPORT_DIR` pointing to it. It writes paired SVG and PDF files for each fixture case. The generic control selects the existing fallback via an unsupported advanced setting.

```sh
export SDOCX_V1_FIXED_EXPORT_DIR="$(mktemp -d /tmp/sdocx-v1-fixed-exports.XXXXXX)"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --features pdf v1_fixed_width --lib
python3 tools/inkpen-v1/compare_fixed_width.py conformance/inkpen-v1-fixed-width-native.json "$SDOCX_V1_FIXED_EXPORT_DIR" /path/to/resvg-helper "$SDOCX_V1_FIXED_EXPORT_DIR.initial.json"
python3 tools/inkpen-v1/compare_fixed_width.py conformance/inkpen-v1-fixed-width-extended-native.json "$SDOCX_V1_FIXED_EXPORT_DIR" /path/to/resvg-helper "$SDOCX_V1_FIXED_EXPORT_DIR.extended.json" --offset 10
```

The comparison requires Pillow, PyMuPDF, pypdfium2, and resvg accepting input SVG and output PNG paths. The retained receipts were regenerated with Pillow 12.1.1, PyMuPDF 1.28.2, pypdfium2 5.14.0, and resvg 0.45.1. Each case requires SVG alpha, MuPDF RGB, and PDFium RGB error to be less than half the generic control's error; PDF is rendered over `#fcfcfc`. Colored-ink, actual saved-note, device, arbitrary-transform, and untested scale parity remain unknown.


## InkPen V1 5–8× host controls

Ten synthetic native Drawing and Mesa controls: width 6, tool 1, curve on, black, three shapes at 5× and 8× and one at 6× and 7×; width 3, tool 0, curve off, bent at 5×; width 12, tool 2, curve on, phase at 8×. Native callback words match Rust and public SVG, MuPDF and PDFium errors are each less than half fallback. Other combinations extrapolate; device and saved-note parity are unverified. The initial eight were recaptured byte-identically with explicit shape flags. Outputs are bounded by existing 65,536-stamp and 8 MiB path limits; candidate files were 19–49 KB PDF and 52–153 KB SVG for the initial eight.

Run from repository root with the dependencies and pinned native capture checkout described above:

```sh
python3 tools/inkpen-v1/high_scale_capture.py /tmp/new-high-scale.json
mkdir /tmp/high-scale-exports
SDOCX_V1_FIXED_EXPORT_DIR=/tmp/high-scale-exports CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --features pdf v1_fixed_width_export_comparison --lib
python3 tools/inkpen-v1/compare_fixed_width.py tools/inkpen-v1/high-scale-native.json /tmp/high-scale-exports /path/to/resvg /tmp/high-scale-comparison.json --offset 21
```

The capture requires fresh output. The comparison requires Pillow, PyMuPDF and pypdfium2; original checks used resvg 0.45.1, PyMuPDF 1.27.2.3, pypdfium2 4.30.0 and Pillow 12.1.1. MuPDF includes the light-paper color conversion baseline. The receipt records absolute errors against transparent native frames for SVG and composited frames for PDF.

SHA-256: native fixture `98401ccd51d896b44308e120c4c78c14754bc05e1babab50e2776d9192d63781`, comparison receipt `97345b3d93f0d6a8104c959a9872d0099a4ca8c559f0e54f9ec1605add241ef9`, capture script `ec6a262341a9a83f89fa27b833366b885f1c11fe9a8dfc1b8949087004ea1713`, comparator `bf4ba7014bcacf5cf77d7ef9ef3cef16fae9262c8402d04183b9dd2efff666ca`. Native library and capture helper pins are recorded in the native fixture.

Four further high-scale color controls cover opaque blue at 5× on a bent path, half-alpha orange at 8× on a phase path, low-alpha purple at 6× on an uncurved bent path, and zero-alpha green at 7× on a straight path. The capture checks native color, tool, curve and fixed-width getters; recapture with these checks produced a byte-identical fixture. The native callback streams match Rust exactly. The public SVG and direct PDF outputs have lower RGB error than generic in resvg, MuPDF and PDFium for each control, against the transparent native readback composited over `#fcfcfc`; the receipt also reports reader-relative PDF backgrounds. The generic SVG uses opaque `#1a1a1a` for every case, ignoring the requested color and alpha, so these differences combine paint and geometry and do not establish geometry-only improvement. The zero-alpha native and candidate frames are transparent. The backdrop is computed from transparent readback, not an independent native opaque-backdrop capture. Other color/path/scale combinations, device rendering, and saved-note parity remain unverified.

```sh
python3 tools/inkpen-v1/high_scale_color_capture.py /tmp/new-high-scale-color.json
mkdir /tmp/high-scale-color-exports
SDOCX_V1_FIXED_EXPORT_DIR=/tmp/high-scale-color-exports CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --features pdf v1_fixed_width_export_comparison --lib
python3 tools/inkpen-v1/compare_high_scale_color.py /tmp/high-scale-color-exports /tmp/high-scale-color-comparison.json
```

The comparison requires resvg, Pillow, PyMuPDF and pypdfium2. Its fixture is pinned to the four controls and the export test's index 31. SHA-256: native fixture `b447b05950fe5d3912cb57a69fa8a4b37abc048eb2c78c60947f7c4f6a80e000`, comparison receipt `1c5b13ef821866202c7f0aec5e57d5b2a06454cd9c9b23a32ee6f50548a4096c`, capture script `6be9065b8d740a148fbda0606d33908ad70666463cb02a320af0a654cae6537e`, comparator `d7b1c5966941a1f641b91f2b0d19340e36058594cf714a4bd9ed2fbdf1801065`.

## Same-input high-scale color comparison

The four high-scale colored native controls were recaptured byte-identically with the project-local runtime and pinned helper checkout described above. The capture script checks the getter snapshots before writing its compact fixture; the snapshots themselves are not retained in that fixture. The crate-local copy of the fixture supports packaged tests and must match the capture fixture byte-for-byte. A separate public export comparison uses exactly the same stroke, including its saved color and fixed-width settings, for both routes. The candidate enables `inkpen_v1_output_scale`; the fallback leaves it unset. At 5×, 8× and 6× the candidate reduces aggregate RGB error against native host frames in resvg, MuPDF and PDFium, including the reader-relative background calculation. The source-alpha-zero 7× case produces identical blank pixels. This comparison isolates the effect of selecting the profile, which changes geometry and coverage together; it does not independently measure either. PDF targets are inferred composites over paper, and synthetic host controls do not establish saved-note, device, or arbitrary-transform parity.

```sh
mkdir /tmp/v1-same-input-exports
SDOCX_V1_SAME_INPUT_EXPORT_DIR=/tmp/v1-same-input-exports CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --features pdf v1_fixed_width_same_input_export_comparison --lib
python3 tools/inkpen-v1/compare_same_input_color.py /tmp/v1-same-input-exports /tmp/v1-same-input-comparison.json
```

The [receipt](same-input-color-comparison.json) records file hashes, versions and absolute as well as reader-relative errors. The original capture command and dependencies are above. Export files are reproducible intermediates and need not be retained.
