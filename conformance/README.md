# Compatibility corpus

Large `.sdocx` fixtures and reference exports live in the
[`twangodev/sdocx-compatibility`](https://huggingface.co/datasets/twangodev/sdocx-compatibility)
dataset on Hugging Face, licensed under CC BY 4.0. The `hf/` Git submodule pins
its revision. Install [Git LFS](https://git-lfs.com/), then initialize the corpus
from the repository root:

```sh
git submodule update --init hf
git -C hf lfs install --local
git -C hf lfs pull
```

Run these commands again after pulling SDK changes to check out the recorded
dataset revision and download its large files. A different checkout can be
selected with `SDOCX_CORPUS_DIR`.

The current dataset revision stores `01-basic-formatting.sdocx` as a regular
Git blob even though its attributes select Git LFS. An LFS-enabled checkout can
report this file as modified without any change to its bytes. Correcting the
dataset's storage metadata requires an upstream commit; the manifest's SHA-256
check still verifies the downloaded source.

The tracked [`corpus.json`](corpus.json) is the versioned lock file. Each fixture
records filenames, SHA-256 digests, page counts and selected parser/layout
expectations. Store the source `.sdocx` and its Samsung-generated reference PDF
side by side in the dataset repository.

Fixture `02-shapes-and-dot-calibration` covers built-in dotted paper, five native
shape paths, one line and 77 handwriting strokes, including twelve pressure/time
calibration marks. Its two stored pages map to one visible page. The detailed
[APK and reference findings](../docs/reverse-engineering/shapes-dot-calibration-findings.md)
record the supported rendering rules and remaining property warnings.

Fixture `04-marker4-highlighter` covers twelve Marker4 strokes, 29 fountain
pen strokes, highlight overlaps and built-in narrow ruled paper. Its two stored
pages map to one visible page and parse without diagnostics.

Run the external corpus locally with:

```sh
cargo test -p sdocx --all-features --test conformance -- --ignored
```

Or point at an existing dataset checkout:

```sh
SDOCX_CORPUS_DIR=/path/to/dataset cargo test -p sdocx --all-features --test conformance -- --ignored
```

Regular unit tests do not download or require private/large fixtures. To add a
fixture, commit and push its artifacts in the Hugging Face dataset, calculate
both SHA-256 digests, then add its entry to `corpus.json`. Commit the updated
`hf` submodule pointer together with the manifest changes in this repository.
The dataset commit must be published before the SDK commit that references it.
See [manifest expectations](manifest-format.md) for optional text checks, exact
page-object counts and diagnostic counts.

## Native geometry checks

The V14 and V16 geometry fixtures (`fountain-v14.json`, `fountain-v16.json`)
check Rust stamp positions, radii, V14 directions and sample boundaries in ordinary CI. Two optional, hash-pinned APK oracles remain
for saved V16 and legacy V14 geometry:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_native.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v14_native.py
```

These require locally extracted libraries and Unicorn. They validate stroke
geometry, not complete native pixel parity. Keep routine validation focused on
production regressions and the real-file visual comparison below.

Marker4 V7 stylus captures in `marker4-v7.json` also run in ordinary Rust CI.
They cover 36 cases, including fractional widths, the minimum radius, short
moves, reversals, taps and stationary input. Its optional native oracle uses
the same hash-checked loader and supports real `ink_geometry` output:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/marker4_native.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/marker4_native.py --prepared geometry.json
```

`ink_visual.py` accepts `--profile`, `--settings`, `--region LEFT TOP RIGHT BOTTOM`
and `--chroma-threshold` to compare selected pen regions. It reports exact ink
overlap, one-pixel-tolerant missing/extra coverage, and mean RGB error in shared
ink. Region selection does not remove other overlapping objects; use matching
isolated layers where available. See the [V7 comparison](../docs/reverse-engineering/marker4-v7.md).

The oracle leaves the null page unmapped. The first library is loaded at base
zero for symbol addressing, but its ELF header must not make null data reads
look valid. Missing required channels still fail validation; absent optional
tilt is passed to native redraw as a null pointer.

The optional real-document WASM check verifies that the debugger retains the
same resolved pen inputs and prepared geometry as export for all 77 fountain
strokes in the calibration document:

```sh
SDOCX_CORPUS_DIR="$PWD/hf" cargo test -p sdocx-wasm --lib \
  debugger::source_tests::replay_and_exports_share_resolved_pen_inputs_and_prepared_geometry \
  -- --ignored --exact
```

Both oracles also accept `--prepared geometry.json` from the `ink_geometry`
example. They select reconstructed fountain strokes of their own profile and
compare Rust's positions, radii and sample boundaries against native redraw;
V14 also compares tangent directions. A mismatch fails the command. Other
pens in a mixed document are excluded, and an empty selection fails.

See [fountain vector parity](../docs/reverse-engineering/fountain-parity.md)
for the verified native geometry and remaining vector-output work.

The intermediate live drawing, GPU, cache, and lifecycle experiments were
retired. Their findings remain in `docs/reverse-engineering/`; their scripts
are available in Git at `40de721`. To recover an individual experiment:

```sh
git show 40de721:conformance/fountain_v17_managed_render.py > /tmp/fountain_v17_managed_render.py
```

Experiments import other scripts from that revision; use a separate checkout
of `40de721` to rerun them together.

## Visual comparison

The local runner validates both file hashes and visible page counts, converts
through the CLI, and compares each page with the Samsung PDF rasterized at the
same size. It writes `report.json`, reference/SDK/difference PNGs, CLI diagnostics
and an HTML report with side-by-side pages and adjustable overlays:

```sh
cargo build -p sdocx-cli
uv run --project conformance --locked python conformance/visual.py \
  --output tmp/visual/current
```

The runner is a dedicated uv project in `conformance/`, with Python 3.14 selected
by [`.python-version`](.python-version), dependencies declared in
[`pyproject.toml`](pyproject.toml), and resolved versions and hashes recorded in
[`uv.lock`](uv.lock). Run these commands from the repository root;
`--project conformance` selects its environment without changing the working
directory. `--locked` checks that the lockfile matches the project.

Use `uv add --project conformance PACKAGE` to add dependencies or
`uv lock --project conformance --upgrade` to update locked versions, then rerun
the synthetic tests below and commit the dependency changes together.

Use a new output directory for each run; existing output is rejected to prevent
stale pages from entering a comparison. `--fixture ID` selects a manifest entry
and may be repeated. `--corpus-dir` (or `SDOCX_CORPUS_DIR`) selects the corpus;
`--cli` selects an explicitly built executable, including one from a baseline
checkout. The runner does not download or modify corpus files.

Pass `--font /path/to/Roboto-Regular.ttf --font /path/to/Roboto-Italic.ttf`
to compare using explicit PNG/PDF font faces. The runner forwards them to the CLI
in order and records their SHA-256 digests separately from system fonts. Use
ordinary installable fonts: PDF-embedded fonts can have stripped Unicode maps
and may be unusable for SVG text even when their family names match.

The report records the executable hash, workspace revision/dirty state, Python
and image-library versions, plus fontconfig file hashes when available. The
workspace revision identifies the reporting checkout, not necessarily the CLI
build. Compare runs with the same toolchain and fonts. Fontconfig inventories
available fonts; it does not prove which font the SVG rasterizer selected.

Metrics include normalized mean absolute RGB error, changed-pixel fraction
(any channel differs by more than 16), and missing/extra ink fractions. Ink is
any channel below 223 against a white or near-white canvas, with one pixel of
matching tolerance. This catches blank output that could score deceptively
well on a mostly empty page. Dark/colored canvases require separate metric
interpretation. Thresholds and tolerance are configurable and recorded.

Only page dimensions are normalized, with at most one raster pixel or half a PDF point of aspect-ratio
rounding; content is never shifted or aligned to improve the score. A mismatch
in hashes, page count or aspect ratio fails the run. Pixel differences are
reported without a universal pass/fail threshold: fonts, antialiasing, line
placement and missing content need different interpretations.

Synthetic runner tests require no external documents or Rust build:

```sh
uv run --project conformance --locked python -m unittest discover \
  -s conformance -p 'test_*.py'
```

To compare the SDK's multipage PDF export, use `--format pdf`:

```sh
cargo build -p sdocx-cli
uv run --project conformance --locked python conformance/visual.py \
  --format pdf --output tmp/visual/pdf \
  --font /path/to/Roboto-Regular.ttf --font /path/to/Roboto-Italic.ttf
```

PDF mode saves one `sdk.pdf`, rasterizes its pages at 96 DPI for comparison,
and records the PDF hash, physical page dimensions, per-page extracted text
and the usual pixel metrics. The HTML report links to the generated PDF.
The source and reference hashes are still checked against the manifest; page
counts must agree before comparison. Omit `--format` to retain PNG mode.

The [PDF export findings](../docs/reverse-engineering/pdf-export-findings.md)
record the current converter, page-size convention, validation and limits.

The initial five-page measurements and font findings are recorded in
[`visual-conformance-findings.md`](../docs/reverse-engineering/visual-conformance-findings.md).
Use the [capture checklist](fixture-capture.md) to prepare the missing native
shape/line, image-placement and standalone-text reference pairs.

## Document composition regressions

The shared Rust text engine has synthetic coverage for paragraph spacing,
ordinary baselines, embedded-object measurement, nested numbered-marker
measurement, and typed vector point/checkbox artwork. Marker widths are
prepared before wrapping; placement, gravity, resolved-size conversion and
retained-font embedding use the shared text pipeline. Body flow still has a
separate cursor loop and page-slice plan.
`text-metrics.json` also stores independently decoded, hash-locked Samsung PDF
observations. Its retained logical canvas height is 848.333333 PDF points;
the reference's actual viewport is 848 points. At scale 1.8, convert a stored
baseline to the actual viewport by subtracting 0.6 SVG units. This conversion
explains the coordinate convention; it does not align content to improve a score.

The current five-page comparison puts matched ordinary baseline Y on the first
four pages within 0.0001 units of the actual viewport. The tests retain their
0.25-unit tolerance. Two locked numbered-marker references now match baseline
Y within 0.000045 units. Numbered-item body X retains an approximately
-0.04393-unit font/advance difference. Table-cell placement and ordinary text
after the continued code block remain outside the passing native subset.
See [text layout evidence](../docs/reverse-engineering/text-layout-findings.md)
for the producers and measured residuals. Recomputed pagination and full
document composition parity remain incomplete. Samsung device-default fonts,
complete point-type cycles and tiny-font serialization precision are also
outside the established coverage.

Rust tests cover stored object order, root render-pass selection, visibility,
nested containers, saved child transforms and replay indices. Export tests
check mixed stroke/image/shape overlaps against literal pixel expectations
and verify PDF paint order, selectable text and vector-only content:

```sh
cargo test -p sdocx --all-features --test composition --test composition_exports
```

These tests use the production Rust parser and renderer. They do not establish
complete Samsung visual parity. Native intersection selection and new captured
mixed-container references remain separate work, described in
[object selection findings](../docs/reverse-engineering/object-selection-findings.md).

## Stroke regressions

Small synthetic tests in `structural_strokes.rs` run in ordinary CI without
external files. They exercise compressed/uncompressed channel order, short and
empty strokes, optional stylus channels, nested objects, multiple layers,
variable mask sizes, unknown extensions, resource limits and malformed records.
Historical native-frame measurements are preserved in
[`fixture-validation.md`](../docs/reverse-engineering/fixture-validation.md).
The retired documents are not part of the current corpus; add new Samsung
stroke/reference-PDF pairs using the [capture checklist](fixture-capture.md).

## Standalone text-box regressions

`structural_text_boxes.rs` adds twelve synthetic archive regressions, including
Unicode, short/empty text, rotation, styles, paragraphs, nested objects,
unsupported-feature diagnostics, malformed boundaries and limits. They run
without external files. SVG checks run with the `render` feature:

```sh
cargo test -p sdocx --all-features --test structural_text_boxes
```

The native frame evidence and current rendering limits are recorded in
[`text-box-findings.md`](../docs/reverse-engineering/text-box-findings.md).
The real corpus still needs a Samsung standalone-text-box export and reference
PDF; synthetic coverage does not establish Samsung visual parity.

## Image and media regressions

`structural_images.rs` has nineteen tests with rendering enabled, and
`media_manifest.rs` has three. They cover explicit ID resolution, reordered and
repeated assets, ambiguous/missing/unsupported references, alternate fill
encodings, bounded frames and records, placement and rotation:

```sh
cargo test -p sdocx --all-features --test structural_images --test media_manifest
```

The `03-image-placement` pair contains seven Chelsea image spans in document
text flow across three visible pages. The corpus checks decoded and resolved
image counts, diagnostics, and three/two/two rendered placements. It covers
resizing, rotation and rectangular cropping. The standalone page-image tests
remain synthetic; this note stores no standalone page images.
See [`image-findings.md`](../docs/reverse-engineering/image-findings.md).

## Shape and line regressions

`structural_shapes.rs` has eighteen synthetic archive tests for geometry,
rotation, fills/outlines, pen references, embedded Unicode text, native paths,
recursive objects, bounds and unsupported-feature diagnostics. SVG checks
cover supported templates, straight lines and quadratic/cubic curves:

```sh
cargo test -p sdocx --all-features --test structural_shapes
```

The object-preservation regression fails on the previous scanner at `3c78cd2`.
Native evidence and current limits are recorded in
[`shape-line-findings.md`](../docs/reverse-engineering/shape-line-findings.md).
Real Samsung shapes/lines with matching reference PDFs are still needed for
visual compatibility coverage.

## Native color checks

`theme-colors.json` contains 785 native lightness-reversal samples checked by
Rust tests. The optional hash-pinned oracle also checks that the native light
theme preserves stored colors:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/theme_native.py
```

See [rendering theme policy](../docs/render-themes.md) for the distinction between
the verified color conversion and export-specific theme selection.
