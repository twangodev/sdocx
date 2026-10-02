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
geometry, not complete native pixel parity. Production regressions and the
real-file visual comparison below check separate rendering contracts.

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
for the verified native geometry and vector appearance limits.

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
The [capture procedure](fixture-capture.md) describes source/reference identity,
metadata and registration requirements.

## Document composition regressions

The shared Rust text engine has synthetic coverage for paragraph spacing,
baselines, embedded-object measurement, numbered markers, body-flow pagination
and selectable vector transport. Its current contracts and limits are documented
in [text vector support](../docs/text-vector-support.md).

Native reference coverage is narrower than synthetic coverage:

| Fixture | Established evidence | Limits |
| --- | --- | --- |
| [`text-metrics.json`](text-metrics.json) | Independently decoded, hash-locked Samsung PDF observations for baselines, markers and clipping. | Table-cell placement and post-code ordinary text are outside the passing native subset. Coordinate conventions and residuals are recorded in [text layout findings](../docs/reverse-engineering/text-layout-findings.md). |
| [`table-ownership.json`](table-ownership.json) | Frame owners and paint-visible cells for 279 native synthetic dense grids; Rust matches the captured outputs. | Covers ownership/visibility, not merged measurement or device-rendered geometry. Sparse/invalid-grid fallbacks report `UnsupportedContent`. |
| [`table-merge-cells.json`](table-merge-cells.json) | Native public merge states for 134 inputs and 1,046 attempts; raw cell identities survive and Rust matches 9,240 owner positions, paint-visible lists and final cold frames. | No attached document/history; text transfer, shaping, complete merged preparation and device appearance remain unverified. Diagnostics and cold text initialization are isolated. See [merge-construction findings](../docs/reverse-engineering/table-code-findings.md#merge-construction-without-an-attached-document). |
| [`table-border-paths.json`](table-border-paths.json) | Model perimeter segments, styles and line equations for twelve native synthetic grids, including merged boundaries and constructor defaults; Rust matches the captured paths. | Model geometry only; canvas painting, complete clipping and device appearance are outside this capture. |
| [`table-border-drawing.json`](table-border-drawing.json) | Native Drawing outline color, width and axis radii for 25 style inputs at seven canvas scales; Rust matches all 175 outputs, including exact width bits. | Captures style aggregation through native Model paths, with a supplied canvas matrix. It does not capture actual line/rectangle painting or device appearance. See [table findings](../docs/reverse-engineering/table-code-findings.md#border-painting). |
| [`table-backgrounds.json`](table-backgrounds.json) | Native cell/table getters agree on 720 selected colors across 40 synthetic grids; Rust matches heading overrides, owned colors and default inheritance. | Model selection only; light/dark SVG/replay/PDF tests check fill alpha and composited text surfaces separately. See [background findings](../docs/reverse-engineering/table-code-findings.md#cell-background-selection). |
| [`table-column-minima.json`](table-column-minima.json) | Native Drawing selects the smallest present cached width, clamps it to the saved per-column minimum, and returns that saved minimum immediately if any layout is missing. Covers 179 synthetic inputs and 182 column queries. | Supplied optional cache widths; no native text shaping, column resizing or merged-frame construction. Rust export retains saved column widths. See [column findings](../docs/reverse-engineering/table-code-findings.md#column-minimum-cache-selection). |
| [`table-cold-frames.json`](table-cold-frames.json) | Native initialization and supplied row updates for 93 inputs, 1,082 slot frames and 532 explicit changes; Rust matches coordinate and pending-gap bits. Includes merged grids and covered-span chains. | Text initialization is isolated; growth decisions, final merged sizing and pagination selection are outside this capture. See [frame-cache findings](../docs/reverse-engineering/table-code-findings.md#cold-frame-cache-and-row-updates). |
| [`table-cold-rows.json`](table-cold-rows.json) | Native cold sizing for 160 inputs and 163 runs; Rust matches 704 raw-cell measurement selections/inputs, split caches, pending gaps and 1,356 resulting slot frames. | Supplied heights with native frame/difference/row routines; text initialization, measurement, update, padding assignment, font selection, synchronization and diagnostics are isolated. Complete merged preparation and device pagination remain unverified. See [cold-sizing findings](../docs/reverse-engineering/table-code-findings.md#cold-row-sizing-from-raw-cell-measurements). |
| [`table-warm-rows.json`](table-warm-rows.json) | Native warm sizing reads frame-owner measurements for 158 inputs and 1,285 slot frames; Rust matches coordinate and pending-gap bits, including small-height thresholds, null layouts and merged owners. | Measured heights are supplied; text initialization is isolated. Native text measurement, complete merged pagination and device appearance are outside this capture. See [warm-sizing findings](../docs/reverse-engineering/table-code-findings.md#warm-row-sizing-from-cached-owner-measurements). |
| [`table-measured-geometry.json`](table-measured-geometry.json) | Native endpoint-owner bounds, edge-width expansion and first-page minima for 154 states, 1,281 slot frames and 914 minimum-height queries; Rust matches `f32` bits. | Supplied frames/text metrics with native getters; text initialization is isolated. Final merged frames, shaping, pagination selection and device appearance are outside this capture. See [geometry findings](../docs/reverse-engineering/table-code-findings.md#measured-bounds-and-first-page-minima). |
| [`table-row-splits.json`](table-row-splits.json) | Native owner-based split propagation and cache replacement for 139 inputs, 1,068 updates and 10,828 cell-cache snapshots; Rust matches changed flags, absent/empty cache states and coordinate bits. | Supplied bands with native lists; text initialization, single-thread mutex and diagnostic interfaces are isolated. Text layout, row movement decisions and complete pagination are outside this capture. See [split-cache findings](../docs/reverse-engineering/table-code-findings.md#row-split-caches-and-warm-text-selection). |
| [`table-row-bottom.json`](table-row-bottom.json) | Native owner-based band selection and row-bottom compression for 148 inputs and 425 updates; Rust matches selected-band, offset, frame and pending-gap bits. Covers merged owners, missing caches/layouts, unsorted bands and boundary comparisons. | Supplied two-line metrics with native getters; text initialization, single-thread mutex and diagnostic interfaces are isolated. Shaping, complete merged preparation and device pagination remain unverified. See [compression findings](../docs/reverse-engineering/table-code-findings.md#row-bottom-compression-from-owner-caches). |
| [`table-warm-control.json`](table-warm-control.json) | Native first-line movement, gap removal, relayout selection and ordered sizing/compression for 148 inputs and 1,089 actions; Rust matches flags, 418 cell selections and 10,892 slot snapshots. | Fixed two-line caches; `layoutCell` records selection without shaping. Initialization, mutex operations and diagnostics are isolated; final observers, native shaping and complete merged preparation are unverified. See [warm-control findings](../docs/reverse-engineering/table-code-findings.md#warm-row-movement-and-relayout-decisions). |
| [`table-cell-inputs.json`](table-cell-inputs.json) | Native Drawing/Widget cell inputs for 138 states and 850 calls; Rust matches raw cached dimensions, integer text bounds, split bands and height-difference bits. | Supplied frames/heights; text shaping, padding assignment, initialization, synchronization, bullet positioning and status are isolated. Complete merged preparation and device appearance remain unverified. See [cell-input findings](../docs/reverse-engineering/table-code-findings.md#cell-layout-frames-and-integer-text-dimensions). |
| [`table-lifecycle.json`](table-lifecycle.json) | Public native `Measure`/`Layout` calls for 69 dense unmerged/merged grids; Rust matches 839 cell selections, 1,665 frame snapshots, split caches, gaps, content/measured bounds and warm first-page minima. Dirty `Layout` and repeated clean `Measure` are checked. | Fixed text caches; text initialization/update/measurement, padding/font selection, diagnostics and final observers are isolated. Native shaping, complete merged preparation and device pagination remain unverified. See [lifecycle findings](../docs/reverse-engineering/table-code-findings.md#public-table-measurement-and-layout-lifecycle). |
| [`table-clipping.json`](table-clipping.json) | Native visible rectangles and actual canvas clip arguments for 55 states, 1,690 queries and 1,240 clip calls; covers merged endpoints, boundary comparisons, band order, signed offsets and outward rounding. | Empty text caches; allocation/fill/move and canvas recording are host supplied. Caller display-rectangle selection, text-pass clipping, Rust clip parity and device appearance remain unverified. See [clipping findings](../docs/reverse-engineering/table-code-findings.md#visible-rectangles-and-canvas-clipping). |
| [`table-export-clipping.json`](table-export-clipping.json) | Native Composer artwork crop and fresh Drawing display defaults for 78 inputs; Rust matches every crop coordinate bit. Covers page/body margins, intersection misses, edge neighbors and outward rounding. | Supplied bounds/page sizes/scaled margins; constructor logging and canvas recording are isolated. No bitmap factory or pixels execute. Text clipping is separate; complete device appearance remains unverified. See [export crop findings](../docs/reverse-engineering/table-code-findings.md#export-artwork-crop). |
| [`table-text-clipping.json`](table-text-clipping.json) | Native Composer per-run clip decisions, world/PDF rectangles and paint translations for 132 inputs, plus the Pdfium empty-clip gate. Exact/adjacent height boundaries, origins/scales, missed intersections and degenerate dimensions repeat across three memory fills. | Supplied Model rectangles, run rectangles and cell origins; host page getter/translation recorder. Native rectangle producers, final PDF paths/pixels and Rust per-run clipping are outside this capture. See [text clip findings](../docs/reverse-engineering/table-code-findings.md#export-text-clips). |
| [`table-text-clip-paths.json`](table-text-clip-paths.json) | Extends all 132 clip cases through actual native Pdfium path construction, rectangle append, matrix transform and shared-path retention: 69 nonempty paths and 345 points, identical across three memory fills. Rust PDF tests separately check supplied clip geometry and selectable text. | Supplied zero rotation and 800-point page height; allocation/memory copy, clip-holder initialization and final append sink are host interfaces. Captures paths before installation; complete native PDF output and Rust per-run clip selection are outside this capture. See [final clip paths](../docs/reverse-engineering/table-code-findings.md#final-pdf-text-clip-paths). |
| [`table-cell-model-bounds.json`](table-cell-model-bounds.json) | Complete native cell/content rectangle setters for 60 cases; captures equal-frame short circuit, independent content bounds, normalization, Model/drawn divergence and dirty flags. Native template/fill/list construction and geometry setters execute unchanged, with three allocation fills. | Zero-seeded supplied objects, nil context/observer, disabled followers and absent images; allocation, mutex construction and diagnostics are host interfaces. Document history, callback mutation and Drawing layout are outside this capture. See [content Model rectangles](../docs/reverse-engineering/table-code-findings.md#cell-content-model-rectangles). |
| [`table-text-bounds.json`](table-text-bounds.json) | Native entry placement and run-bound producers for 162 cases, 810 entries and 378 unions, plus 22 native grouping probes across three memory fills. Rust line bounds/baselines match within 0.0001 units. | Supplied entry/block/line metrics, logical maps and ordinary spans; host memory copy. Font wrappers use native null-implementation ID -1. Shaping, wrap selection, actual objects, complete run emission, font-specific gates and Rust per-run clipping are outside this capture. See [retained text bounds](../docs/reverse-engineering/table-code-findings.md#retained-text-entry-and-run-bounds). |
| [`table-text-runs.json`](table-text-runs.json) | Complete native cached-glyph run emission for 230 cases, 1,116 supplied entries, 528 emitted runs and 1,180 output codewords across three memory fills. Covers font/style splits, bitmap/language gates, RTL reversal, multi-glyph entries, subranges, offsets, gravity and style overrides. Rust run tops/bottoms/baselines match within 0.0001 units. | Supplied metrics, glyph caches, spans and font getters; host allocation/deletion/copy/move. Native shaping/selection, actual embedded objects, emoji slices, Composer clipping, final PDF painting and Rust per-run clipping are outside this capture. See [complete run emission](../docs/reverse-engineering/table-code-findings.md#complete-retained-text-run-emission). |
| [`table-text-span-identity.json`](table-text-span-identity.json) | Native Widget span defaults and property conversion, Model getters, Base string operations and Text semantic equality for 70 cases across three memory fills. Separately allocated equal font names compare equal; correction/composition fields affect identity. | Supplied property buffers, conversion order, size delta/scale and explicit test theme mapping. Serialized decoding, native span traversal, shaping and rendering are outside this capture. See [span identity](../docs/reverse-engineering/text-draw-identity-findings.md#span-identity). |
| [`table-text-span-binary.json`](table-text-span-binary.json) | Native factory, complete span readers/writers/sizes, WDoc header/buffer checks and Base strings/lists for 237 version-7/8 records across three memory fills. Captures UTF-8/CESU-8 font names, suggestion lists, read-only composition spans, rejected correction persistence and partial mutations on truncated records. | Supplied bounded records and available byte counts; host allocation/memory/string-length/mutex interfaces. Font-name writer reserved bytes are explicitly prezeroed; native suggestion consumed counters retain their unit/byte discrepancy. Whole documents, Widget conversion, shaping and rendering are outside this capture. See [binary boundaries](../docs/reverse-engineering/text-draw-identity-findings.md#native-binary-boundaries). |
| [`table-font-metadata.json`](table-font-metadata.json) | Native font getters and CBDT table-tag scan for 60 supplied metadata cases across three memory fills. Covers null paths, tag positions, 32-tag capacity, source-ID sentinels and short/allocated languages. | Supplied cached source metadata, interface vtables and typeface table tags. Font factory, family selection, language construction and shaping are outside this capture. See [font metadata](../docs/reverse-engineering/text-draw-identity-findings.md#font-metadata). |
| [`table-text-ownership.json`](table-text-ownership.json) | Native entry initialization, supplied shaped-glyph ownership production, placement and complete retained-run emission for 38 cases, 124 UTF-16 entries, 94 glyphs and 40 runs across three memory fills. Continuation slots preserve source ranges without splitting real glyph runs; owned zero-advance/zero-ink glyphs and first-entry versus later-union degenerate bounds are captured separately. | Shaped source-to-owner maps, advances, glyph metrics, spans, font interfaces and bidi order are supplied. Leading unowned entries can produce a separate empty run. Native shaping, font selection, wrapping, embedded objects and final painting are outside this capture. See [UTF-16 entry ownership](../docs/reverse-engineering/text-draw-identity-findings.md#utf-16-entry-ownership). |

Native cold-row, warm-row and measured-geometry captures each check sixteen
saved table-height-limit inputs against an otherwise identical uncapped grid.
Both flag values and maxima 0/1/10/1000 leave the captured outputs unchanged.
The [height-limit findings](../docs/reverse-engineering/table-code-findings.md#table-wide-height-limit)
distinguish this export contract from Model's editing-time resize.

Prepared table regressions cover bounded dense unmerged/merged grids, cold/warm
row sizing, endpoint-owner outlines, page gaps and both split modes. Paged merged
owners are compared with an unmerged projection at the same saved bounds.
Sparse grids, rotated/nested child objects and partial horizontal obstacles are
outside this prepared subset. Native merged parent placement and device
pagination remain unverified.
Complete document composition, Samsung device-default fonts, complete point-type
cycles and tiny-font serialization precision remain unverified.

Rust tests cover stored object order, root render-pass selection, visibility,
nested containers, saved child transforms and replay indices. Export tests
check mixed stroke/image/shape overlaps against literal pixel expectations
and verify PDF paint order, selectable text and vector-only content:

```sh
cargo test -p sdocx --all-features --test composition --test composition_exports
```

These tests use the production Rust parser and renderer. They do not establish
complete Samsung visual parity. Native intersection selection and captured
mixed-container reference coverage are described in
[object selection findings](../docs/reverse-engineering/object-selection-findings.md).

## Stroke regressions

Small synthetic tests in `structural_strokes.rs` run in ordinary CI without
external files. They exercise compressed/uncompressed channel order, short and
empty strokes, optional stylus channels, nested objects, multiple layers,
variable mask sizes, unknown extensions, resource limits and malformed records.
Historical native-frame measurements are preserved in
[`fixture-validation.md`](../docs/reverse-engineering/fixture-validation.md).
The retired documents are not part of the current corpus.

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
The corpus contains no Samsung standalone-text-box/reference-PDF pair;
synthetic coverage does not establish Samsung visual parity.

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

Native evidence and current limits are recorded in
[`shape-line-findings.md`](../docs/reverse-engineering/shape-line-findings.md).
The `02-shapes-and-dot-calibration` pair contains five native shapes and one
line. Other templates, dashes and arrowhead settings lack reference coverage.

## Native color checks

`theme-colors.json` contains 785 native lightness-reversal samples checked by
Rust tests. The optional hash-pinned oracle also checks that the native light
theme preserves stored colors:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/theme_native.py
```

See [rendering theme policy](../docs/render-themes.md) for the distinction between
the verified color conversion and export-specific theme selection.
