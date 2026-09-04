# Compatibility corpus

Large `.sdocx` fixtures and reference exports live outside this Git repository.
The external dataset is
[`twangodev/sdocx-compatibility`](https://huggingface.co/datasets/twangodev/sdocx-compatibility)
on Hugging Face and is licensed under CC BY 4.0. Check out or download that
dataset into the ignored `hf/` directory at the repository root. A different
checkout can be selected with `SDOCX_CORPUS_DIR`.

The tracked [`corpus.tsv`](corpus.tsv) is the lock file. Each row records the
fixture filenames, SHA-256 digests, and a small set of stable parser/layout
expectations. Store the source `.sdocx` and its Samsung-generated reference PDF
side by side in the dataset repository.

Run the external corpus locally with:

```sh
cargo test -p sdocx --test conformance -- --ignored
```

Or point at an existing dataset checkout:

```sh
SDOCX_CORPUS_DIR=/path/to/dataset cargo test -p sdocx --test conformance -- --ignored
```

Regular unit tests do not download or require private/large fixtures. To add a
fixture, upload its artifacts to the dataset, calculate both SHA-256 digests,
then append one tab-separated row to `corpus.tsv`.

## Visual comparison

The local runner validates both file hashes and visible page counts, converts
through the CLI, and compares each page with the Samsung PDF rasterized at the
same size. It writes `report.json`, reference/SDK/difference PNGs, CLI diagnostics
and an HTML report with side-by-side pages and adjustable overlays:

```sh
cargo build -p sdocx-cli
uv run --locked --only-group conformance python conformance/visual.py \
  --output tmp/visual/current
```

Use a new output directory for each run; existing output is rejected to prevent
stale pages from entering a comparison. `--fixture ID` selects a manifest row
and may be repeated. `--corpus-dir` (or `SDOCX_CORPUS_DIR`) selects the corpus;
`--cli` selects an explicitly built executable, including one from a baseline
checkout. The runner does not download or modify corpus files.

Pass `--font /path/to/Roboto-Regular.ttf --font /path/to/Roboto-Italic.ttf`
to compare using explicit PNG font faces. The runner forwards them to the CLI
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

Only page dimensions are normalized, with at most one pixel of aspect-ratio
rounding; content is never shifted or aligned to improve the score. A mismatch
in hashes, page count or aspect ratio fails the run. Pixel differences are
reported without a universal pass/fail threshold: fonts, antialiasing, line
placement and missing content need different interpretations.

Synthetic runner tests require no external documents or Rust build:

```sh
uv run --locked --only-group conformance python -m unittest discover \
  -s conformance -p 'test_*.py'
```

## Handwritten stroke regressions

[`strokes.tsv`](strokes.tsv) locks the three original handwritten documents by
SHA-256 and records the independent native-frame audit's stroke/point counts.
They are not included in the current Hugging Face corpus. A full Git checkout
can recover the exact original blobs into ignored local storage:

```sh
mkdir -p tmp/stroke-conformance
while IFS="$(printf '\t')" read -r filename digest blob strokes points; do
  case "$filename" in \#*|'') continue ;; esac
  git cat-file blob "$blob" > "tmp/stroke-conformance/$filename"
done < conformance/strokes.tsv
SDOCX_STROKE_CORPUS_DIR="$PWD/tmp/stroke-conformance" \
  cargo test -p sdocx --test stroke_conformance -- --ignored
```

Use an absolute corpus directory because Cargo runs integration tests from the
crate directory. The test checks all 7,182 strokes and 924,442 points, channel
lengths, and coordinates against each stroke's independently stored bounding
rectangle. It fails on the former shifted-layout decoder: handwritten alone
produces 322,406 points instead of the audited 321,776.

Small synthetic tests in `structural_strokes.rs` run in ordinary CI without
external files. They exercise compressed/uncompressed channel order, short and
empty strokes, optional stylus channels, nested objects, multiple layers,
variable mask sizes, unknown extensions, resource limits and malformed records.
The handwritten corpus is structural/geometry coverage; it does not establish
pixel-for-pixel equivalence with Samsung rendering.

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

`structural_images.rs` has twelve tests with rendering enabled, and
`media_manifest.rs` has three. They cover explicit ID resolution, reordered and
repeated assets, ambiguous/missing/unsupported references, alternate fill
encodings, bounded frames and records, placement and rotation:

```sh
cargo test -p sdocx --all-features --test structural_images --test media_manifest
```

The handwritten corpus runner also validates the 21 native media manifest
entries and hashes across its three locked documents. The standalone-image
tests are synthetic; real Samsung image/reference-PDF coverage is still needed.
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
