# Legacy Pencil translated-frame counterexamples

These two synthetic Drawing GLV1 host-Mesa captures use the same local native prerequisites, seed, framebuffer, pressure ramp, background injection, and limitations as [the color controls](../old-pencil-color/README.md). Neither is a device or saved-note capture. They translate the four source coordinates; `translation_only` starts at (143.25, 102.5) and `offset125` starts at (143.125, 102.125). Both produce 42 records. The pre-draw frames are byte-identical to `conformance/old-pencil-color/varied-pre.rgba.zlib`, which is shared rather than duplicated. The retained VBOs and compressed post frames are top-origin RGBA8 1920 × 1080; `receipt.json` pins raw frame and VBO SHA-256, full original capture JSON and exact source hashes, and GL errors. JSON hashes record the particular capture, which contains variable runtime data; the full JSON is not retained here.

From the repository root, generate the original control source and apply the retained patches:

```sh
python3 tools/old-pencil-color/make_controls.py "$PWD" /tmp/sdocx-pencil-repro
patch -o /tmp/sdocx-pencil-repro/translation_only.py /tmp/sdocx-pencil-repro/varied.py < tools/old-pencil-counterexample/translation_only.patch
patch -o /tmp/sdocx-pencil-repro/offset125.py /tmp/sdocx-pencil-repro/varied.py < tools/old-pencil-counterexample/offset125.patch
patch -o /tmp/sdocx-pencil-repro/stamps.py /tmp/sdocx-pencil-repro/translation_only.py < tools/old-pencil-counterexample/stamps.patch
patch -o /tmp/sdocx-pencil-repro/stamps125.py /tmp/sdocx-pencil-repro/offset125.py < tools/old-pencil-counterexample/stamps125.patch
```

Run each generated script with the same `PYTHONPATH`, `--capture`, `--initial-seed 1`, and `--output` invocation given in the color-control README. The two stamp scripts alter the host replay: each draw advances attribute 1 (three F32s, offset 16*i) and attribute 2 (one F32, offset 16*i+12), retains divisor 1, draws one instance, and reads a pixel. Both full replay frames matched their corresponding original instanced draws byte for byte. `stamp-replay-receipt.json` records full-frame hashes, pixel changes and a digest of all 42 pixel observations, zero GL errors, source and native JSON hashes; raw logs and JSON are not retained. These replays localized historical discrepancies at a stamp edge and an interior stamp sample; they do not establish the host rasterizer's exact arithmetic.

The current regression compares every pixel under all 12 existing paint profiles and requires exact agreement for both translated frames. The historical one-pixel differences motivated these retained controls; they are not current test failures. Public export, zoom, and device parity remain unverified. The first translated VBO matched the source sampler bit for bit in an isolated test; the public saved-note bridge remains unproven.

## Additional synthetic controls

Further host controls cover pressure, color, serialized Model inputs, reflection, priors and overlap. Their local capture scaffolding is outside this tracked reproduction workflow. The [legacy Pencil findings](../../docs/reverse-engineering/old-pencil-color-findings.md) summarize the admission boundaries and historical counterexamples.

Injected-quads and modified-shader sweeps reject simple interpolation hypotheses without proving original-shader arithmetic. Original-shader host compiler IR exports vertex texture coordinates; constant-white edge controls match nearest-even 1/256-pixel snapping. Twelve injected regions match the source-side profile, while Triangle0 differs at nine pixels in one width-7 region. Shader replacement can change compilation. Size-8 capture metadata contained a stale color; the executed SetPenData color and uniform identify the actual input.

Synthetic serialized inputs cover distance gates, size, reflection, stretched geometry, transparent priors, dots and rapid bends. Name-table resolution uses an altered Fountain-origin object, not a real saved Pencil note. Reflection leaves stored bounds stale. The tracked `size12_visible_dot_decoded_channels_match_native_getters` regression checks the visible-dot channels. Controlled overlap captures preserve equal 31-instance streams and first-post/second-pre continuity; separate two-color captures were independently reproduced. These observations do not establish production scheduling. Host projection overrides are interventions, not saved-page zoom.

## Maintained reader checks and historical diagnostics

The ignored `old_pencil_pdf_vector` native-frame tests compare fallback and opt-in direct PDF using pinned PyMuPDF 1.28.2, pypdfium2 5.9.0, PDFium 150.0.7869.0 and Pillow. Run with project agent build settings and both `SDOCX_PDF_ORACLE_PYTHON` and `SDOCX_PDFIUM_ORACLE_PYTHON` pointing to the pinned interpreter:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SDOCX_PDF_ORACLE_PYTHON=/path/to/python SDOCX_PDFIUM_ORACLE_PYTHON=/path/to/python cargo test -p sdocx --features pdf --test old_pencil_pdf_vector direct_pdf_matches_retained_native -- --ignored
```


The experimental shading, shifted-image-mask and diagnostic dot exporters/comparators have been retired. Their bounded measurements are preserved in [the historical findings](../../docs/reverse-engineering/old-pencil-color-findings.md#retired-pdf-and-svg-diagnostics), not offered as replay workflows. Red-on-green and shifted-phase SVG comparisons composited separately rendered transparent layers over native priors; they do not establish in-process canvas parity. The original-phase red PDF counterexample does not justify removing the single-stamp fallback.
