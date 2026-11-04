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

The Rust tests verify full vector words, replay boundaries, and invalid-width fallback. Historical public SVG and PDF comparisons reduced error against generic controls; the reader comparison scaffolding has been retired. Actual saved-note, device, arbitrary-transform, and untested scale parity remain unknown.

## InkPen V1 5–8× host controls

Ten synthetic native Drawing and Mesa controls: width 6, tool 1, curve on, black, three shapes at 5× and 8× and one at 6× and 7×; width 3, tool 0, curve off, bent at 5×; width 12, tool 2, curve on, phase at 8×. Native callback words match Rust and public SVG, MuPDF and PDFium errors are each less than half fallback. Other combinations extrapolate; device and saved-note parity are unverified. The initial eight were recaptured byte-identically with explicit shape flags. Outputs are bounded by existing 65,536-stamp and 8 MiB path limits; candidate files were 19–49 KB PDF and 52–153 KB SVG for the initial eight.

Recapture the high-scale controls with `python3 tools/inkpen-v1/high_scale_capture.py /tmp/new-high-scale.json`. Native library and helper pins are recorded in the native fixture. MuPDF comparisons included the light-paper color conversion baseline.

Four further high-scale color controls cover opaque blue at 5× on a bent path, half-alpha orange at 8× on a phase path, low-alpha purple at 6× on an uncurved bent path, and zero-alpha green at 7× on a straight path. The capture checks native color, tool, curve and fixed-width getters; recapture with these checks produced a byte-identical fixture. The native callback streams match Rust exactly. The public SVG and direct PDF outputs have lower RGB error than generic in resvg, MuPDF and PDFium for each control, against the transparent native readback composited over `#fcfcfc`; the receipt also reports reader-relative PDF backgrounds. The generic SVG uses opaque `#1a1a1a` for every case, ignoring the requested color and alpha, so these differences combine paint and geometry and do not establish geometry-only improvement. The zero-alpha native and candidate frames are transparent. The backdrop is computed from transparent readback, not an independent native opaque-backdrop capture. Other color/path/scale combinations, device rendering, and saved-note parity remain unverified.

Recapture with `python3 tools/inkpen-v1/high_scale_color_capture.py /tmp/new-high-scale-color.json` using a fresh output path.

## Same-input high-scale color comparison

The four high-scale colored native controls were recaptured byte-identically with the project-local runtime and pinned helper checkout described above. The capture script checks the getter snapshots before writing its compact fixture; the snapshots themselves are not retained in that fixture. The crate-local copy of the fixture supports packaged tests and must match the capture fixture byte-for-byte. A separate public export comparison uses exactly the same stroke, including its saved color and fixed-width settings, for both routes. The candidate enables `inkpen_v1_output_scale`; the fallback leaves it unset. At 5×, 8× and 6× the candidate reduces aggregate RGB error against native host frames in resvg, MuPDF and PDFium, including the reader-relative background calculation. The source-alpha-zero 7× case produces identical blank pixels. This comparison isolates the effect of selecting the profile, which changes geometry and coverage together; it does not independently measure either. PDF targets are inferred composites over paper, and synthetic host controls do not establish saved-note, device, or arbitrary-transform parity.

The reader comparison above is historical; its optional comparator and derived receipt have been retired. Native inputs and capture scripts remain retained.
