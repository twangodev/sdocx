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
