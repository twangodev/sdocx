# Shapes and dot calibration fixture

Investigated `hf/02-shapes-and-dot-calibration.{sdocx,pdf}` on 2026-09-21,
against parent revision `b0b33d5`. This is an investigation of the existing parser
and renderer, not a renderer fix.

## Fixture and reproduction

- SDOCX SHA-256: `dd6580a0531cf1712ef6526f1b0d51c42adaeb0d11feacca5d23ba562239da82`.
- PDF SHA-256: `2971d07e8aeb8e63d6322d535d4db91311e54830482506828b67273f22e0e507`.
- Two stored pages, one visible reference page. Source page size: 1848 × 2613.
- Reference PDF page: 600 × 848 points. Its background image extends to
  848.3766 points and is clipped by the PDF page boundary.
- Visible source content: 77 strokes, five native shapes, and one native line.

The findings use `parse_detailed` with default options and
`render_document_svg` with default render options. To reproduce the visible
output with the CLI:

```sh
cargo run --offline -p sdocx-cli -- hf/02-shapes-and-dot-calibration.sdocx \
  -o /tmp/02-shapes-and-dot-calibration.svg
```

PDF measurements below use PyMuPDF to inspect image placements and rasterize
both reference and generated SVG at matching source-coordinate scale.

## Dotted background: recognized metadata, missing rendering

Both stored pages decode to `PageTemplate { id: 7, source: BuiltIn }` with
background RGB `(252, 252, 252)`. For this fixture, built-in template 7 is the
light-gray dotted background visible in the PDF.

The first page's flexible property mask is `0x671`. After its 32-byte content
bounds, the known fields contain:

| Property bit | Meaning | Value |
| --- | --- | --- |
| 4 | Background image mode | 2 |
| 5 | Background ARGB | `0xfffcfcfc` |
| 6 | Background width | 1848 |
| 9 | Built-in template type | 7 |

Bit 10 is also set. There are 55 bytes after the known fields before the layer
collection; their semantics were not established by this investigation. The
current property decoder stops at bit 9. The empty second page has mask `0x270`,
the same four known values, and no trailing property bytes. This means bit 10 is
not required to identify template 7 in this file.

`parse_page_properties` retains the template ID. `render_page_contents_svg`
paints the background color but never uses `page.template` to draw a pattern.
The archive has no embedded PNG/JPEG/PDF background asset: its media consists of
`mediaInfo.dat` and a page SPI stream. Consequently the pattern cannot be restored
by exposing an overlooked image entry. There is also no template-specific
unsupported-rendering diagnostic in the current report.

The PDF contains a separate 1800 × 2545 JPEG for the dotted background, placed at
`(0, 0, 600, 848.3766)` points. Measuring the gray-dot rows/columns in that image
(threshold <230, grouping contiguous active rows/columns) gives:

| Measurement | PDF points | Source coordinates |
| --- | ---: | ---: |
| Horizontal pitch, fitted excluding clipped first column | 29.772 | 91.698 |
| Vertical pitch | 27.001 | 83.164 |
| Horizontal grid origin, fitted | -0.191 | -0.589 |
| First vertical row center | 17.834 | 54.929 |

There are 21 visible columns, including a clipped left-edge column, and 31 rows.
These are **measurements of this export**, not established universal constants
for Samsung's template 7. In particular, assuming a square grid would disagree
with this reference. Exact template sizing, anchoring, and color behavior should
be established before generalizing support to other page dimensions or themes.

## Hand-drawn calibration dots: decoded, with rendering differences

The 4-column × 3-row calibration marks are 12 ordinary strokes, indices 47–58
(zero-based in the visible page's decoded stroke array). They contain 297–462
samples each, with pressure and time channels and repeated sample coordinates.
They are not single-point strokes or unrecognized object types.

Their bounds centers are near source x = 93, 182, 273, 365 and
source y = 719, 803, 887. Comparing each center to its nearest background-grid
intersection gives horizontal residuals from -2.285 to +2.768 and vertical
residuals from -2.104 to +3.024 source units. This supports consistent placement
between decoded strokes and the reference background.

All 12 are visible in the current SVG. The rendered marks are somewhat thinner
than Samsung's export. At a common source-coordinate raster scale, thresholding
each isolated dot's RGB channels below 128 yields reference bounds of roughly
10–13 × 9–16 pixels, versus 9–13 × 8–15 pixels in our output. Several differ by
1–2 pixels. Rasterization, antialiasing, and the current approximate pressure
width model affect that comparison; it does not by itself prove a coordinate
or pressure-decoding error. The calibration mark width difference should be
studied separately from the missing background.

## Other gaps exposed by this fixture

The five native shape types are rectangle (4), ellipse (1), triangle (2),
pentagon (11), and hexagon (6). The renderer handles the first three but returns
without drawing types 11 and 6. Their handwritten labels still render because
they are separate strokes. The diagonal native line also renders.

The current parser emits five `UnsupportedShapeFeature` warnings, one for each
shape. The pentagon and hexagon warnings include `shape template`; the first
three also carry geometry-extension/path warnings despite their visible fallback
geometry. All five have retained path data, including 90 bytes for the pentagon
and 107 for the hexagon, which provide a concrete starting point for additional
shape support.

## Follow-up implementation boundaries

1. Establish and render the template-7 background through the shared SVG page
   renderer, so normal viewing, replay backgrounds, and exports agree. Avoid
   treating these measured spacings as universal without further evidence.
2. Add supported native shape paths/templates for the pentagon and hexagon with
   fixture-level visual coverage.
3. Use the twelve marks to evaluate pressure/width fidelity independently. Do
   not change decoded coordinates merely to compensate for a missing pattern
   or a stroke-paint approximation.

## APK-backed implementation: page layout and native dimensions

Samsung Notes 4.4.45.37 arm64 `libSPenComposer.so`,
`NotePDFExporterVectorList::exportPages` (`0x361864`), obtains the complete
`WNote::GetPageList`, subtracts one from its count at `0x3618d0`, and iterates
only those preceding pages. This is list-mode compatibility storage, not a
body-text-dependent blank-page heuristic. The 02 end tag explicitly records
page mode 0 (LIST); its flow height is `2 * 2613 + 41 = 5267`.

The SDK now exposes raw page mode and orientation from the already bounded end
tag decoder. Layout recognizes list-mode compatibility only with a complete
flow-height/padding match, at least two records, an empty decoded final page,
and matching final/previous dimensions, template and background. Intermediate
blank or template-only pages remain visible; a one-page blank note remains
one page. Continuous/unknown modes and ambiguous final backgrounds are retained.
The older text-only fallback is limited to notes with no page-mode metadata.
This deliberately supports less than the native exporter's unconditional final
record exclusion. Raw pages and source-page indices are unchanged.

`libSPenWDoc.so`, `WNoteLoadHandler::loadNoteFile_FixedData`
(`0xa9290–0xa92f0`) reads two optional `u32` dimensions after the body object
when bytes remain before the flexible offset, into members 176/180. These are
the default page dimensions, not the multi-page flow canvas. The decoder keeps
the original trailing bytes and exposes the dimensions separately.
`WNote::GetDocumentDensity` (`0x9ec70`) selects width for portrait, height for
landscape, then divides by 360. This supplies native template scaling without
inferring a scale from the exported PDF.

The manifest now locks the 02 hashes, two stored/one visible page, 77 strokes,
five shapes, one line, and five retained geometry-property warnings. All three
locked corpus pairs pass structural and reference page-count checks. The deleted
01 PDF in the working dataset was left untouched; that check used its local LFS
object in a temporary corpus directory.

## APK-backed dot renderer

The Java `SpenWPage` constants identify narrow/medium/wide dots as 7/8/9.
The arm64 `libSPenComposer.so` drawing path establishes:

| Symbol/address | Rule |
| --- | --- |
| `DotTemplateDrawing::getLineHeightSize`, `0x3f2494`, table `0x20d10c` | Native sizes 12, 17, 28. |
| `TemplateDrawingBase::GetLineHeight`, `0x3f3d44` | Vertical pitch = size × 1.35 × document density. |
| `DotTemplateDrawing::Draw`, `0x3f2244` | One-row bitmap with height `ceil(pitch)`; round zero-length dashes, horizontal pitch = bitmap height + 1.5 × density. |
| `0x3f232c–0x3f2358` | Light color `#010102`, dark color `#fafafa`, alpha 0.2. |
| `0x3f23ac–0x3f2418` | Diameter 2 × density, minimum one native pixel at unit scale; first center at x=0, y=diameter/2. |
| `TemplateDrawing::onDraw`, `0x3f1f18` | Repeat bitmap, scale Y by pitch / bitmap height; add constant 65 to the top before scaling. |
| `libSPenContent.so`, constant table `0x7e00`, `CalculatePixels` `0x1331c` | Constant 65 is 10 document-density units, with no integer rounding. |

`page_background.rs` derives this geometry from native default dimensions and
orientation. The renderer emits one dashed SVG path after the solid background and before
all ink/objects, with one horizontal subpath per row and round zero-length
dashes. A fractional Y scale preserves the native bitmap's vertical adjustment.
It does not reproduce Samsung's bitmap antialiasing or zoom-dependent
minimum-pixel widening. SVG size scales with rows, not dots or zoom; configurations
above 10,000 rows are rejected with a diagnostic to bound rendering work. CLI SVG,
PNG/PDF export, WASM viewing and replay backgrounds share this implementation.
No separate web template renderer or cache is added.

At density `1848 / 360`, the native formula gives pitches 91.7 × 83.160004,
agreeing with the reference measurements above within rasterization rounding.
The supported IDs are only the three variants confirmed by the same native
class. Other templates, custom URI/image/PDF backgrounds, rotated dots, or
missing native dimensions/orientation retain metadata and produce
`UnsupportedPageTemplate`; they keep the solid background. Raw background image
ID, mode, width, rotation and template URI are now exposed rather than discarded.
Mode/width belong to background-image handling and do not determine built-in dot
spacing. Unknown template IDs retain the full `u32` value.

`TemplatePDFWriter::writeDot` (`0x38425c`) uses different vector-export color,
radius and spacing rules. The 02 PDF contains a JPEG background, so this change
follows the native drawing/capture path; it does not conflate the two exporters.


Raster validation caught an exporter detail: resvg 0.47 rounds SVG pattern-tile
sizes to integer pixels (`render_pattern_pixmap` in its `src/path.rs`). A
91.7 × 83.16 repeating tile became 92 × 83, causing cumulative drift. Explicit
row subpaths preserve fractional coordinates in browser, PNG and PDF rendering
without adding a second renderer. The 02 visual comparison at 1848 × 2613 now
reports 0.85% changed pixels, 1.25% missing ink and 0.72% extra ink (one-pixel
matching tolerance). Remaining differences include bitmap antialiasing and the
existing pressure-width model; calibration stroke widths were not changed.

The Samsung PDF MediaBox is 600 × 848 points while its captured background is
848.3766 points tall. The comparison runner allows one raster pixel or half a
PDF point of page-size rounding; a larger mismatch still fails. Content is not
translated or aligned to reduce the error.

Final validation also covers CLI PDF output (0.87% changed pixels, 1.58% missing
ink, 0.63% extra ink), a PNG regression sampling distant grid intersections,
and the actual WASM viewer/replay path in Chromium and Firefox. The browser
check verifies that both views share identical template geometry, all six
native shape/line paths survive the replay background, the clear top margin
renders correctly, and the second stored page stays inspectable without a
second visible preview. WebKit could not launch on this host because its system
libraries are missing. Workspace all-feature tests, parser-only checks, Clippy,
formatting, web type checks, 39 web unit tests and the production build pass.
