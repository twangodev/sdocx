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
| [`table-grid-admission.json`](table-grid-admission.json) | Native raw/public cell and frame-owner lookup for 25 supplied topologies and 117 queries across three memory fills. Records 76 pointer reads beyond declared row lengths and 26 stops immediately before null-candidate span dereferences. | Initialized backing capacity extends beyond shortened vector ends. Null cases stop before the unsafe read. Native construction, binary loading and supported sparse document/editor admission do not execute. See [dense construction and sparse transport](../docs/reverse-engineering/table-code-findings.md#dense-editor-construction-and-sparse-transport). |
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
| [`table-cell-drawing.json`](table-cell-drawing.json) | Complete native artwork pass for 60 cases, 738 Canvas commands and 501 selected paths; Rust matches captured path/translation fields, including normal versus constrained edges. | Theme/paint/outline and measured inputs are supplied; no text shaping, native pixels or complete composition. See [cell artwork](../docs/reverse-engineering/table-code-findings.md#complete-cell-artwork-pass). |
| [`table-cell-model-lifecycle.json`](table-cell-model-lifecycle.json) | Actual Model table/row/cell/content construction, Drawing frame preparation and Bodytext Model bridge for 17 cases across three allocation fills. Independent replay agrees. | Drawing/Bodytext storage and text adapter are supplied. Full listener callbacks, ObjectSpan origins, first text measurement, parsing/cloning and PDF run clipping are excluded. See [cell Model bridge](../docs/reverse-engineering/table-code-findings.md#cell-model-bridge-after-frame-preparation). |
| [`table-code-layout.json`](table-code-layout.json) | Complete native Measure/measured-bounds/minimum/ClearMeasure for 18 cold/warm/cleared cases; Rust matches all cold and cleared geometry bits with supplied child metrics. | Source bounds, child layout/height/first-line interfaces and density-scaled constants are supplied. Warm native cache retention is captured separately; complete Rust warm-cache/child-placement/pagination parity is not established. See [code chrome](../docs/reverse-engineering/table-code-findings.md#code-block-chrome-and-split-inputs). |
| [`table-page-text-ranges.json`](table-page-text-ranges.json) | Complete native boundary predicates and page range scan for 47 cases and 251 queries; inclusive UTF-16 ranges, overlap and exhausted/absent sections repeat across three allocation fills. | Page/line/cursor/first-empty getter values are supplied. No measured paragraph producer, document repagination or production measured-line indexer. See [page ranges](../docs/reverse-engineering/text-layout-findings.md#captured-page-text-ranges). |
| [`table-text-paragraph-layout.json`](table-text-paragraph-layout.json) | Complete native paragraph layout chain with actual constructors, host ICU76.1/Unicode16 bidi/breaking, 25 cases, 38 lines and 81 placed UTF-16 entries; captures conditional natural widths and surrogate-cut emergency ranges. | Source, measured advances, kinds, metrics, spacing and ink are supplied. No font measurement/classification, objects/bullets, obstacles, pagination, clips, vector export or Android device ICU parity. Production retains separate SDK break/grapheme/height policy. See [paragraph loop](../docs/reverse-engineering/text-layout-findings.md#captured-paragraph-layout-loop). |
| [`table-text-font-registry.json`](table-text-font-registry.json) | Actual initialized XML/four-file registries and NAME resolution for 154 requests; records source/parent/implementation reuse classes, native language/CBDT getters and copy cleanup. | Four supplied XML configurations and process-local counter seed; no generic device registry, arbitrary registration/invalidation, glyph/cache records or full retained-run grouping. See [live registry](../docs/reverse-engineering/text-draw-identity-findings.md#captured-live-font-registry). |
| [`table-cell-model-bounds.json`](table-cell-model-bounds.json) | Complete native cell/content rectangle setters for 60 cases; captures equal-frame short circuit, independent content bounds, normalization, Model/drawn divergence and dirty flags. Native template/fill/list construction and geometry setters execute unchanged, with three allocation fills. | Zero-seeded supplied objects, nil context/observer, disabled followers and absent images; allocation, mutex construction and diagnostics are host interfaces. Document history, callback mutation and Drawing layout are outside this capture. See [content Model rectangles](../docs/reverse-engineering/table-code-findings.md#cell-content-model-rectangles). |
| [`table-text-bounds.json`](table-text-bounds.json) | Native entry placement and run-bound producers for 162 cases, 810 entries and 378 unions, plus 22 native grouping probes across three memory fills. Rust local vertical bounds/baselines match exact f32 bits. | Supplied entry/block/line metrics, logical maps and ordinary spans; host memory copy. Font wrappers use native null-implementation ID -1. Shaping, wrap selection, actual objects, complete run emission, font-specific gates and Rust per-run clipping are outside this capture. See [retained text bounds](../docs/reverse-engineering/table-code-findings.md#retained-text-entry-and-run-bounds). |
| [`table-text-wrap-numeric.json`](table-text-wrap-numeric.json) | Complete native `GetBlockInfo` and `SetLayout` for 17 cases, 59 supplied UTF-16 slots, 55 candidate operand/result pairs, 16 commits and 45 selected/placed slots. Rust matches numeric selection/width/cursor fields, including through the production slot bridge; three fills and independent replay agree. | Advances/kinds/break ends, rectangles, old positions, ink, visual map and single-line shell are supplied. Shaping, ICU break/bidi production, automatic native paragraph wrapping, natural-width ceiling, draw clipping, full composition and output do not execute. Rust UAX/grapheme/height policy remains separate. See [ordinary wrap arithmetic](../docs/reverse-engineering/text-layout-findings.md#captured-ordinary-wrap-arithmetic). |
| [`table-text-context-windows.json`](table-text-context-windows.json) | Native separator and `A-unit-B` scans for all 65536 UTF-16 units; 32 valid source cases, 215 offset queries, 1390 directional selections and 2564 iterator windows/actual caller TextViews. Rust matches the exhaustive scans and every emitted view; three fills and independent replay agree. | Paragraph view, selection and direction are supplied. Full paragraph measurement, cache lookup, LayoutPiece/HarfBuzz and the partial-measure fast path do not execute. See [cache-word context](../docs/reverse-engineering/text-layout-findings.md#captured-cache-word-context-windows). |
| [`table-text-runs.json`](table-text-runs.json) | Complete native cached-glyph run emission for 230 cases, 1,116 supplied entries, 528 emitted runs and 1,180 output codewords across three memory fills. Covers font/style splits, bitmap/language gates, RTL reversal, multi-glyph entries, subranges, offsets, gravity and style overrides. Rust local run tops/bottoms/baselines match exact f32 bits. | Supplied metrics, glyph caches, spans and font getters; host allocation/deletion/copy/move. Native shaping/selection, actual embedded objects, emoji slices, Composer clipping, final PDF painting and Rust per-run clipping are outside this capture. See [complete run emission](../docs/reverse-engineering/table-code-findings.md#complete-retained-text-run-emission). |
| [`table-text-cached-runs.json`](table-text-cached-runs.json), [`table-text-cached-ownership.json`](table-text-cached-ownership.json) | Repeat the 230 run and 38 ownership cases with explicit native cached-input snapshots immediately before emission: 1,240 UTF-16 entries and 568 output records. Preserve the original captured run outputs while recording advances, kinds/directions, cache flags, glyph payloads, complete spans and supplied font metadata. | Supplied shaping, placement inputs and font interfaces remain distinct from native emission. Opaque codewords are not established Rust SFNT glyph IDs. Empty default records do not establish a safe native first-glyph read. See [cached-entry snapshots](../docs/reverse-engineering/text-draw-identity-findings.md#captured-cached-entry-snapshots). |
| [`table-text-cached-object-runs.json`](table-text-cached-object-runs.json) | Eighty-two native object scenarios with cached-entry snapshots: 56 published records and 80 supplied codewords, plus 26 missing-upstream-glyph controls. The original 80 scenarios are preserved; two cache overrides verify kind-5 nonempty false-drawable and null-font branches. | Shape records, owner maps, font interfaces and line metrics are supplied. Typed kernel comparisons include the 56 published cases; the 26 empty controls fault at an attempted null first-word read in deliberately unmapped guest memory, without establishing production cache or application-crash behavior. Actual shaping, font selection, complete outer composition and PDF painting remain excluded. See [cached-entry snapshots](../docs/reverse-engineering/text-draw-identity-findings.md#captured-cached-entry-snapshots). |
| [`table-text-decorations.json`](table-text-decorations.json) | Complete native text-paint setup and decoration drawing for 368 supplied cases and 438 rectangle commands across three memory fills. Captures ordinary/suggestion/correction thickness, font-unit conversion, style/link gates, color/alpha arguments and reversed endpoints. | Retained spans/configuration, entry metrics/direction and opacity are supplied; host paint interfaces/copy/destruction and rectangle recorder. Actual SkPaint alpha merging, font selection, shaping, themes, full glyph drawing, Composer decoration painting and pixels are outside this capture. See [decoration commands](../docs/reverse-engineering/text-layout-findings.md#captured-decoration-commands-and-paint-inputs). |
| [`table-text-span-identity.json`](table-text-span-identity.json) | Native Widget span defaults and property conversion, Model getters, Base string operations and Text semantic equality for 70 cases across three memory fills. Separately allocated equal font names compare equal; correction/composition fields affect identity. | Supplied property buffers, conversion order, size delta/scale and explicit test theme mapping. Serialized decoding, native span traversal, shaping and rendering are outside this capture. See [span identity](../docs/reverse-engineering/text-draw-identity-findings.md#span-identity). |
| [`table-text-measurement-join.json`](table-text-measurement-join.json) | Complete native measurement-join predicate and native String construction/comparison for 291 supplied cases across three memory fills, with forward/reverse/self comparisons. Captures f32 size semantics, complete foreground ARGB, all 256 style bytes under mask `0xc3`, object exclusion and null/empty/semantic-name identity. | Supplied span members and bounded strings; host allocation/memory/string-length interfaces. Widget/theme conversion, document parsing, shaping, resolved-face selection, full measurement-run construction and painting do not execute. See [measurement identity](../docs/reverse-engineering/text-draw-identity-findings.md#measurement-identity). |
| [`table-text-predefined-style.json`](table-text-predefined-style.json) | Native global style assignment, complete predefined FontSize/Bold span factory and getters for ten cases, plus five paragraph-application cases across three memory fills. Heading 1/2/3 produce sizes 21/19/15 with bold true; Body produces 15/bold false, with interval 3. | Paragraph-to-text mapping and text length are supplied. `AppendSpan` and paragraph-list generation are intercepted; native editing/history/storage, Widget conversion, measurement and rendering do not execute. Seeded factory records establish append order, not effective precedence. See [predefined spans](../docs/reverse-engineering/text-layout-findings.md#predefined-style-span-factory). |
| [`table-text-object-runs.json`](table-text-object-runs.json) | Native measurement initialization, `SpanRunFunctor` glyph/object window, placement, cached retrieval and complete public retained emission for 54 published object records; 26 isolated-helper controls capture a missing-upstream-glyph precondition. All 80 cases repeat across three memory fills. Object/layout/ordinary-background members survive emission; composing background is omitted. | Shaped owner records, cached fonts, mapped spans/context and line/block metrics are supplied. Native font selection/shaping, full Widget/upstream dispatch, metric production/wrapping, Composer policy/painting and output do not execute. Isolated empty caches are not production cache or application-crash evidence; Body callers filter objects separately. See [retained object runs](../docs/reverse-engineering/text-draw-identity-findings.md#captured-retained-object-runs). |
| [`table-text-object-background.json`](table-text-object-background.json) | Native object-span measurement, placement and complete preview background painting for 40 cases and 37 rectangle commands across three memory fills. Captures inline margin-inclusive width versus block visible width, leading-inclusive bands, NaN fallback, zero-width dispatch and mapped-color selection. | Supplied geometry/font metrics/context and line/block metrics derived from native object measurements; host paint/memory/rectangle interfaces. Native `GetBlockInfo`, wrapping/obstacles, Widget object conversion, theme mapping, retained-run emission, Composer/PDF policy and pixels do not execute. Rust paints supported measured object bands; unsafe positions retain scoped diagnostics. See [object background geometry](../docs/reverse-engineering/text-draw-identity-findings.md#captured-embedded-object-background-geometry). |
| [`table-text-object-export-policy.json`](table-text-object-export-policy.json) | Fourteen native Composer caller/gate windows for 204 cases across three memory fills. Body background callers skip objects and dispatch them separately; Table/Code/Placed callers preserve object background requests. Captures legacy background-alpha gates and the newer shared foreground-alpha gate independently. | Supplied DrawnText fields, null/non-null pointers and bounded stack/register state. Background/object/text calls are recorded and intercepted; full callers, object/font producers, PDF allocation/clipping/path painting and pixels do not execute. See [object export caller policy](../docs/reverse-engineering/text-draw-identity-findings.md#captured-object-export-caller-policy). |
| [`table-text-pdf-alpha.json`](table-text-pdf-alpha.json) | Sixty native Composer/Pdf paint cases across three memory fills: complete color/alpha setter chains, raw paint getters and Pdfium RGBA extraction. Captures Table's signed foreground-alpha threshold, Code's alpha replacement and a single background-alpha product. | Supplied retained colors, writer opacity, paint storage and wrapper interfaces. Final FPDF fill installation is intercepted; full writer selection, font/text production, glyph drawing and output do not execute. Body/Placed absence of later alpha replacement is a separate source finding. See [PDF alpha transport](../docs/reverse-engineering/text-draw-identity-findings.md#captured-pdf-alpha-transport). |
| [`table-text-background-theme.json`](table-text-background-theme.json) | Complete native dark-theme construction and HSL color conversion for 12 ARGB sentinel cases across three memory fills. Transparent black maps to nonzero transparent white; transparent white maps to zero. | Supplied colors and host libc `fmod` only. Recorded nonzero predicates describe color integers; native span conversion, effective-background selection, painting, density and geometry do not execute. General Rust/native RGB rounding parity is outside this capture. See [background consumers](../docs/reverse-engineering/text-draw-identity-findings.md#preview-and-composer-backgrounds). |
| [`table-text-span-binary.json`](table-text-span-binary.json) | Native factory, complete span readers/writers/sizes, WDoc header/buffer checks and Base strings/lists for 237 version-7/8 records across three memory fills. Captures UTF-8/CESU-8 font names, suggestion lists, read-only composition spans, rejected correction persistence and partial mutations on truncated records. | Supplied bounded records and available byte counts; host allocation/memory/string-length/mutex interfaces. Font-name writer reserved bytes are explicitly prezeroed; native suggestion consumed counters retain their unit/byte discrepancy. Whole documents, Widget conversion, shaping and rendering are outside this capture. See [binary boundaries](../docs/reverse-engineering/text-draw-identity-findings.md#native-binary-boundaries). |
| [`table-font-metadata.json`](table-font-metadata.json) | Native font getters and CBDT table-tag scan for 60 supplied metadata cases across three memory fills. Covers null paths, tag positions, 32-tag capacity, source-ID sentinels and short/allocated languages. | Supplied cached source metadata, interface vtables and typeface table tags. Font factory, family selection, language construction and shaping are outside this capture. See [font metadata](../docs/reverse-engineering/text-draw-identity-findings.md#font-metadata). |
| [`table-text-font-language.json`](table-text-font-language.json) | Actual bundled libxml parsing and native family-language propagation for 26 XML cases and 39 intercepted font records across three memory fills. Captures missing/empty language, exact strings, entity decoding and duplicate records. | Native `readFont` is intercepted before creating fonts and returns false. Actual device font configuration, font selection, source-instance allocation, shaping and rendering do not execute. See [font-family language](../docs/reverse-engineering/text-draw-identity-findings.md#captured-font-family-language). |
| [`table-text-font-source.json`](table-text-font-source.json) | Native file-font construction, Skia/bundled FreeType parsing, source ID/copy getters and bitmap scan for 24 cases and 48 independent sources. Same-file constructors produce distinct process-counter IDs; copies preserve source identity. The supplied pinned Roboto face has 19 parsed SFNT tags and no CBDT. | Host file adapters supply pinned font bytes; language and counter history are explicit inputs. Device configuration, selection/manager reuse, shaping/synthesis, parser-error paths, actual bitmap font files and rendering are excluded. See [file-font source instances](../docs/reverse-engineering/text-draw-identity-findings.md#captured-file-font-source-instances). |
| [`table-text-shaping.json`](table-text-shaping.json) | Native file-font/family/collection construction, layout-piece production, bundled HarfBuzz and Skia/FreeType measurement for 16 cases: 16 shaping calls, 48 glyphs, 49 UTF-16 advances and 48 vector metric samples across three memory fills. Records pre/post shaping state, scale/ppem, owner positions, ink and profile-dependent advances. Rust matches all captured glyph IDs and UTF-16 owner starts with the recorded shaping inputs. | One supplied pinned Roboto face and caller paint/locale/range/direction; host ICU 76.1 property/locale services, libc/libm and allowlisted native comparators through host qsort. No scalar advance callback executes. Device selection/fallback, whole measurement producer, wrapping/composition, vector output and document-integrated Rust metric parity are excluded. See [native shaping capture](../docs/reverse-engineering/text-layout-findings.md#captured-native-shaping). |
| [`table-text-shaping-numeric.json`](table-text-shaping-numeric.json) | Independent extension through the same native producer: 17 cases, 19 shaping calls, 182 glyphs, 182 UTF-16 advances and 182 vector metric samples. Captures large paint scales, a long pen accumulation, Latin/Greek/Cyrillic chunks, both directions, skew, positive/negative spacing and a nonzero UTF-16 request start after a supplementary scalar. Rust matches all captured glyph IDs and UTF-16 owners. | Shares the pinned font, native libraries and host service boundaries of the original fixture. The mixed-script case records three actual shaping chunks; separate [producer/chunk geometry regressions](../docs/reverse-engineering/text-layout-findings.md#bounded-rust-paint-shaping) reproduce their metrics and geometry. Full paragraph production, device font selection and arbitrary-font shaping parity remain excluded. See [numeric geometry evidence](../docs/reverse-engineering/text-layout-findings.md#captured-post-shaping-numeric-geometry). |
| [`table-text-shaping-gpos.json`](table-text-shaping-gpos.json) | Four native shaping cases with 12 glyphs/UTF-16 advances, four actual horizontal GPOS adjustment traces and 12 local fused-skew operations. Rust regressions verify source bytes in the pinned font, signed multiplier/product/shift and destination advances; a local mark-offset witness distinguishes native fused skew from separately rounded arithmetic. | Same supplied-font producer and host boundaries. The adjustment trace covers only GPOS `ValueFormat=4` horizontal `x_advance`; other trace formats, device/variation corrections and full paragraph shaping remain excluded. Separate bounded Rust producer regressions reproduce these captured shaping outputs. See [GPOS and skew evidence](../docs/reverse-engineering/text-layout-findings.md#captured-horizontal-gpos-scaling-and-fused-skew). |
| [`table-text-shaping-skia-metrics.json`](table-text-shaping-skia-metrics.json) | Eighteen native supplied-Roboto cases with 108 raw advances and 432 ink coordinates, plus actual residual/fixed matrices, hint/load flags, cached fixed-advance transitions and FreeType outline points before/after transformation. Covers both skew signs, fractional/tiny sizes and normalization boundaries across three memory fills. | Same supplied-font producer and host boundaries. First-use cached fields remain null until an executed store. No pixels, other fonts, variable/bitmap fonts or document composition parity. See [Skia metric evidence](../docs/reverse-engineering/text-layout-findings.md#captured-skia-residual-matrices-and-outline-metrics). |
| [`table-text-shaping-mixed-scripts.json`](table-text-shaping-mixed-scripts.json) | Thirty-three native cases, 380 shape calls and 800 glyphs/UTF-16 advance entries with Latin/Greek/Cyrillic chunk discovery, shared f32 pen, per-call owner origins and half-spacing stores. RTL preserves ascending call ranges and reverses owners within each chunk. Three fills and independent replays agree. | Same supplied single-face producer and host boundaries. Full paragraph bidi resolution, fallback selection, whole SpanRunFunctor, entry conversion, wrapping and document composition do not execute. See [mixed-script evidence](../docs/reverse-engineering/text-layout-findings.md#captured-mixed-script-chunk-geometry). |
| [`table-text-entry-geometry.json`](table-text-entry-geometry.json) | Eight native cases, eight shape calls, 25 glyphs, 30 source UTF-16 slots and 64 actual paint profiles. Executes complete layout append, bounded SpanRun glyph/width/ink conversion and actual RectF empty/scale/union calls across three fills. | Layout origin, zero extra advance and preallocated entry/glyph storage are supplied. Paint profile setters do not establish shaping-face selection. Whole SpanRunFunctor, manager initialization, editing, entry classification, wrapping, fallback and document composition remain excluded. See [logical-entry evidence](../docs/reverse-engineering/text-layout-findings.md#captured-logical-entry-conversion-and-paint-profiles). |
| [`table-text-shaping-entry-skia-metrics.json`](table-text-shaping-entry-skia-metrics.json) | Eleven native cases, 11 shape calls, 26 glyphs/UTF-16 advances, 11 scaler configurations, 50 FreeType loads and two fast advances around supplied raw paint size 1712.5. Actual flags and outline points distinguish Mono hinting from normal-target hinting. | Raw paint size is supplied independently of the informational source size; source-size multiplication is proven in the separate entry fixture. Whole SpanRunFunctor, typeface selection, raster output and arbitrary-font equivalence remain excluded. See [fractional hinting evidence](../docs/reverse-engineering/text-layout-findings.md#captured-fractional-paint-hinting). |
| [`table-text-shaping-itemization.json`](table-text-shaping-itemization.json) | Forty-four native cases, 314 plain-Script queries, 102 shape calls, 252 glyphs and 264 UTF-16 advances. Actual script-loop results retain Common/Inherited absorption, finalized chunk ranges and five-scalar full-source contexts, including partial ranges and neutral missing-glyph output. | Supplied pinned Roboto, direction and source range; same native/host boundaries. Script_Extensions, full bidi resolution, malformed UTF-16, device fallback and version-independent Unicode properties remain excluded. See [itemization findings](../docs/reverse-engineering/text-layout-findings.md#captured-script-itemization). |
| [`table-text-span-paint.json`](table-text-span-paint.json) | Ninety-three profiles through complete native family registration, Typeface creation/getters and span paint helper. Captures source size/style setters, threshold/tiny sizes and final Typeface weight/italic overriding the initial 400/false; allocation fills and independent drivers agree. | One supplied regular Roboto family and requested metadata; consumed getter bits exclude unwritten padding. Font-name/XML/system-default resolution, physical style-face selection, synthesis metrics, whole SpanRunFunctor, shaping, fallback and composition remain excluded. See [complete span paint findings](../docs/reverse-engineering/text-layout-findings.md#captured-complete-span-paint-helper). |
| [`table-text-span-font-name.json`](table-text-span-font-name.json) | Native FontManager/XML parsing, span NAME/default selection, style-name parser, physical best-match, complete paint helper and typeface setter for 130 profiles. Pins null versus empty NAME, four physical Roboto styles and caller direction; typed Rust resolution matches every profile. Three fills and independent replay agree. | One supplied XML configuration/four pinned files, source style/size, nullable names and direction; bounded host file/property/ICU services. Generic caller database matching is not native parity. Device configuration, alternate parser paths, fallback glyph choice, shaping/metrics, fake-bold metrics, later whole-span execution and composition remain excluded. See [NAME/default findings](../docs/reverse-engineering/text-layout-findings.md#captured-span-font-name-and-default-selection). |
| [`table-text-shaping-named-faces.json`](table-text-shaping-named-faces.json) | Actual NAME-selected physical face and complete span paint feed LayoutPiece/HarfBuzz/Skia/FreeType and bounded entry conversion for 110 profiles, 134 calls, 437 glyphs/advances, 614 FT loads and 110 scaler configs. Rust matches 106 pieces/122 calls/401 glyphs and explicitly rejects four skew-composite profiles; 429 raw advances/bounds match. Independent repeats agree. | The same supplied XML/four pinned files and bounded manager host services; no extra font slot/fallback. Production paragraph admission remains Regular-only. Fake-bold measurement, arbitrary/device configuration, later whole-span execution, wrapping/composition and output are excluded. See [named-face measurement](../docs/reverse-engineering/text-layout-findings.md#captured-named-face-measurement). |
| [`table-text-shaping-consumer-metrics.json`](table-text-shaping-consumer-metrics.json) | Actual NAME/paint/LayoutPiece/HarfBuzz/Skia/FreeType and bounded entry conversion for 52 Regular/normal/LTR profiles, 52 calls, 134 glyphs/UTF-16 slots, 186 loads and 52 scaler configs. Rust matches every profile without rejection; three fills and independent byte replays agree. | Supplied marker/body strings at ten sizes plus two `AB` controls under pinned XML/fonts. Native list numbering, object feedback, wrapping/composition, other physical styles and output do not execute. See [consumer text metrics](../docs/reverse-engineering/text-layout-findings.md#captured-consumer-text-metrics). |
| [`table-text-ownership.json`](table-text-ownership.json) | Native entry initialization, supplied shaped-glyph ownership production, placement and complete retained-run emission for 38 cases, 124 UTF-16 entries, 94 glyphs and 40 runs across three memory fills. Continuation slots preserve source ranges without splitting real glyph runs; owned zero-advance/zero-ink glyphs and first-entry versus later-union degenerate bounds are captured separately. | Shaped source-to-owner maps, advances, glyph metrics, spans, font interfaces and bidi order are supplied. Leading unowned entries can produce a separate empty run. Native shaping, font selection, wrapping, embedded objects and final painting are outside this capture. See [UTF-16 entry ownership](../docs/reverse-engineering/text-draw-identity-findings.md#utf-16-entry-ownership). |
| [`table-text-owner-bases.json`](table-text-owner-bases.json) | Native constructors and supplied shaped-owner producer for nine cases, 70 UTF-16 entries, 18 owned slots and 21 glyph records across three memory fills. Captures nonzero request starts separately from source-vector base, including source-space classification after subtracting base three. | Source/ranges, relative owners, glyph IDs/positions/ink, advances and font wrappers are supplied. Minikin/HarfBuzz and chunk owner normalization, font selection, native bidi assignment, placement, emission and painting do not execute. See [nonzero owner bases](../docs/reverse-engineering/text-draw-identity-findings.md#captured-nonzero-owner-bases). |

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

The [physical-face transport regressions](../crates/sdocx/tests/svg_face_identity.rs)
check selected Regular/Bold identities under synthetic styling through typed
usvg resolution and retained/raw SVG PDF outlines. The
[transport findings](../docs/reverse-engineering/text-layout-findings.md#svg-physical-face-transport)
separate source-byte/collection identity from native measurement admission and
Samsung appearance. A separate [standalone Chromium probe](../web/tests/e2e/svg-face-identity.spec.ts)
observes actual Regular/Bold PostScript identities; it does not establish
inline SVG in the application DOM:

```sh
cargo test -p sdocx --all-features --test svg_face_identity
```

### Native wrapping numeric replay

The [`--text-wrap-numeric` capture](native_table/text_wrap_numeric.rs) uses the
native driver built below and extracted Model/Base/Text libraries. It executes
ordinary block selection and placement with supplied entry data:

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-wrap-numeric scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-text-wrap-numeric.json
cmp /tmp/table-text-wrap-numeric.json conformance/table-text-wrap-numeric.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-context-windows scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-text-context-windows.json
cmp /tmp/table-text-context-windows.json conformance/table-text-context-windows.json
```

### Native shaping replay

The optional [`--text-shaping` capture](native_table/text_shaping.rs) requires
the hash-pinned extracted ARM64 libraries, Unicorn, the bundled supplied font
and the exact host ICU/libc/libm versions recorded in
[`table-text-shaping.json`](table-text-shaping.json). It verifies those hashes
before execution. Run from the repository root:

```sh
rustc --edition 2024 -D warnings -C panic=abort conformance/native_table.rs \
  -L native=scratch/apk-analysis-runtime/python/unicorn/lib \
  -C link-arg=-Wl,-rpath,"$PWD/scratch/apk-analysis-runtime/python/unicorn/lib" \
  -o /tmp/sdocx-native-table
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping.json
cmp /tmp/table-text-shaping.json conformance/table-text-shaping.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-numeric scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping-numeric.json
cmp /tmp/table-text-shaping-numeric.json conformance/table-text-shaping-numeric.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-gpos scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping-gpos.json
cmp /tmp/table-text-shaping-gpos.json conformance/table-text-shaping-gpos.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-skia-metrics scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping-skia-metrics.json
cmp /tmp/table-text-shaping-skia-metrics.json conformance/table-text-shaping-skia-metrics.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-mixed-scripts scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping-mixed-scripts.json
cmp /tmp/table-text-shaping-mixed-scripts.json conformance/table-text-shaping-mixed-scripts.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-entry-geometry scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-entry-geometry.json
cmp /tmp/table-text-entry-geometry.json conformance/table-text-entry-geometry.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-entry-skia-metrics scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping-entry-skia-metrics.json
cmp /tmp/table-text-shaping-entry-skia-metrics.json conformance/table-text-shaping-entry-skia-metrics.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-itemization scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-shaping-itemization.json
cmp /tmp/table-text-shaping-itemization.json conformance/table-text-shaping-itemization.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-span-paint scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf > /tmp/table-text-span-paint.json
cmp /tmp/table-text-span-paint.json conformance/table-text-span-paint.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-span-font-name scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-text-span-font-name.json
cmp /tmp/table-text-span-font-name.json conformance/table-text-span-font-name.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-named-faces scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-text-shaping-named-faces.json
cmp /tmp/table-text-shaping-named-faces.json conformance/table-text-shaping-named-faces.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-shaping-consumer-metrics scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-text-shaping-consumer-metrics.json
cmp /tmp/table-text-shaping-consumer-metrics.json conformance/table-text-shaping-consumer-metrics.json
```

The [shaping findings](../docs/reverse-engineering/text-layout-findings.md#captured-native-shaping)
distinguish executed native code, supplied caller inputs and host services.
This replay does not establish device font selection or production Rust shaped
advance parity.
The [post-shaping comparisons](../docs/reverse-engineering/text-layout-findings.md#captured-post-shaping-numeric-geometry)
use captured HarfBuzz output and Skia bounds to verify exact f32 geometry for
37 cases across the original, numeric and GPOS fixtures. The
[mixed-script comparisons](../docs/reverse-engineering/text-layout-findings.md#captured-mixed-script-chunk-geometry)
add 33 cases, 380 shape calls and 800 glyphs with continuous pen accumulation.
The separate
[paint-metric comparisons](../docs/reverse-engineering/text-layout-findings.md#bounded-rust-paint-metrics)
generate 401 raw advances and 1604 ink coordinates with Rust/Skrifa and match
the captured bits for 74 cases across six metric fixtures. The
[Skia trace comparisons](../docs/reverse-engineering/text-layout-findings.md#captured-skia-residual-matrices-and-outline-metrics)
also check fixed cached advances and 4420 original/transformed point coordinates.
The [paint-sized producer comparisons](../docs/reverse-engineering/text-layout-findings.md#bounded-rust-paint-shaping)
derive metrics in Rust and match all 558 native shape calls/1453 glyphs across
eight fixtures. Public post-shaping layout also matches all 1453 glyphs,
including contiguous Latin/Greek/Cyrillic/Common chunks in both directions.
The [entry conversion capture](../docs/reverse-engineering/text-layout-findings.md#captured-logical-entry-conversion-and-paint-profiles)
separately executes native layout append and bounded SpanRun instruction
windows for 25 glyphs, 30 source UTF-16 slots and 64 actual paint profiles.
Public Rust producer/layout/entry conversion matches all eight entry cases.
The [fractional raw-scaler capture](../docs/reverse-engineering/text-layout-findings.md#captured-fractional-paint-hinting)
pins the native Mono hint target with 11 cases and 26 glyphs; its raw paint
inputs are distinct from the entry fixture's source-size conversion.
The configured runtime hinter also matches 192 fixed-coordinate extrema from
48 hinted native glyph loads before outward bbox rounding; this is separate
from ordered point-stream equality.
The [itemization capture](../docs/reverse-engineering/text-layout-findings.md#captured-script-itemization)
records 44 cases and 314 actual plain-Script queries; `PaintItemization`
matches chunk ranges and capped full-source context. Public whole-piece
measurement matches generated shape requests and shared layout/entry geometry
for all 151 cases across the eight suites. The
[complete span paint capture](../docs/reverse-engineering/text-layout-findings.md#captured-complete-span-paint-helper)
separately establishes 93 final paint profiles and the later Typeface style
override; it does not execute shaping or physical style synthesis.
These comparisons remain separate from production paragraph measurement and
do not establish complete native document width/wrapping parity.

### Native composition and registry replay

After building `/tmp/sdocx-native-table` as above, replay these supplied-input
captures with the extracted libraries:

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-drawing scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-cell-drawing.json
cmp /tmp/table-cell-drawing.json conformance/table-cell-drawing.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-model-lifecycle scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-cell-model-lifecycle.json
cmp /tmp/table-cell-model-lifecycle.json conformance/table-cell-model-lifecycle.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --code-layout scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-code-layout.json
cmp /tmp/table-code-layout.json conformance/table-code-layout.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --page-text-ranges scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so > /tmp/table-page-text-ranges.json
cmp /tmp/table-page-text-ranges.json conformance/table-page-text-ranges.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-paragraph-layout scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-text-paragraph-layout.json
cmp /tmp/table-text-paragraph-layout.json conformance/table-text-paragraph-layout.json
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --font-registry scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-text-font-registry.json
cmp /tmp/table-text-font-registry.json conformance/table-text-font-registry.json
```

The fixture rows and linked findings distinguish executed native routines from
supplied storage, metrics and host interfaces. These replays do not establish
complete document composition or device appearance.

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
