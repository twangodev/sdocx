# SVG-based PDF export

## Implementation and scope

The optional Rust `pdf` feature exports the existing visible-page SVGs into one
PDF. `render_document_pdf` uses the shared document layout and render options;
`render_svg_pages_pdf` accepts previously rendered pages in slice order.
The CLI supports `--format pdf`, `.pdf` output inference, repeated `--font`
paths and `--pdf-dpi`. Explicit format selection takes precedence over the
filename extension. PDF writes one document even when the note has many pages.

The implementation uses [krilla 0.8.2](https://docs.rs/krilla/0.8.2/krilla/)
and [krilla-svg 0.8.1](https://docs.rs/krilla-svg/0.8.1/krilla_svg/), with
usvg/resvg 0.47.0 shared with PNG export. All workspace packages now declare
Rust 1.92. PDF dependencies remain optional for library consumers; the WASM
bindings continue to use only the `render` and `serde` features.

The supplied font database is shared across all pages. CLI font precedence
matches PNG: explicit faces are loaded before system faces. Available fonts
are embedded and text retains Unicode mappings. The SDK's default PDF options
discover system fonts; `PdfOptions::new` accepts a caller-controlled database.

## Physical size and error behavior

PDF uses 72 points per inch. The default export scale is 96 SVG coordinate
units per inch, so a 1080 × 1527 page becomes 810 × 1145.25 points. This is a
documented export convention, not a decoded Samsung print setting. Changing
`PdfOptions::dpi` or `--pdf-dpi` changes physical size without rasterizing paths.

The Samsung reference is 600 × 848 points. Setting `--pdf-dpi 129.6` gives
600 × 848.333 points for its SDK page dimensions: the slight height difference
comes from the stored canvas aspect ratio. The exporter preserves that ratio
instead of inferring a paper size from this one fixture.

Empty documents, nonfinite/nonpositive DPI, invalid SVG, inconsistent SVG/page
dimensions, and page sizes outside PDF's 14,400-point limit return errors.
PNG data is decoded before conversion and has a 64 MiB output-buffer limit.
External SVG image file paths are not loaded. Conversion completes in memory
before the CLI opens its output path, so conversion/font errors do not replace
an existing output. Filesystem write failures remain possible.

## Measured reference comparison

Validated on 2026-09-04 with the hash-locked `01-basic-formatting` corpus pair,
PyMuPDF 1.28.2, Pillow 12.1.1 and NumPy 2.4.2. Regular and italic Roboto files
use the hashes recorded in the [font findings](visual-conformance-findings.md).
The runner verifies source hashes and compares pages at 1080 × 1527 pixels.
No content alignment or shifting is applied.

The generated PDF contains five visible pages, is 61,964 bytes, and has
extractable text on every page. Its resources contain embedded TrueType font
subsets and no image objects for this text fixture. The main regular/italic
faces are Roboto; the Unicode/code samples also use system DejaVu fallbacks.

| Page | Changed pixels | Missing reference ink | Extracted characters |
| ---: | ---: | ---: | ---: |
| 1 | 4.42% | 7.74% | 482 |
| 2 | 1.71% | 1.52% | 426 |
| 3 | 4.10% | 22.98% | 313 |
| 4 | 2.92% | 9.09% | 243 |
| 5 | 1.99% | 0.60% | 274 |

Mean changed-pixel fraction is 3.03%; normalized mean absolute RGB error is
0.01604. The corresponding PNG baseline measured 3.28% and 0.01365. Different
rasterizers affect these scores, so they are not evidence of a universal
fidelity improvement. Unicode font coverage remains the largest visible gap.
Text extraction confirms selectable content, not preservation of every source
character or semantic reading order for every document.

Visual inspection of the first and final PDF pages confirms the formatting,
links' appearance, and clipped continuation of the code block. PDF annotations
are not present for those links. Separate synthetic graphics checks exercised
a rectangular clip, overlapping 50%-opacity fills, and a 90-degree rotation.
Compared with resvg PNGs, both graphics pages had zero changed pixels at the
runner's 16-channel-value threshold and zero missing/extra ink. Interior color
samples agreed within two channel values. These are SVG/PDF integration checks,
not Samsung shape/image fidelity evidence.

## Validation and remaining limits

- Five SDK PDF tests decode exported bytes with an independent PDF parser:
  page order, mixed sizes, scale, selectable text, embedded fonts/images,
  vector paths, trailing-storage-page omission, color options and invalid input.
- CLI tests exercise actual processes and files: multipage output naming,
  extension/flag precedence, custom scale, invalid fonts/DPI and preservation
  of existing output on conversion failure.
- Fourteen Python runner tests pass, including PDF page order/counts, physical
  dimensions, extracted text, artifact hashes and report download links. The
  exact locked CI command also passes in an isolated environment.
- Workspace tests, strict Clippy, formatting, the Rust 1.92 all-targets check,
  a parser-only build and the browser WASM target check pass locally.
- All five PNG output hashes still match the pre-PDF baseline with the same
  supplied fonts. Hosted CI has not run for these local commits.

PDF export inherits the shared SVG renderer's limitations: unsupported native
shapes, image effects and incomplete Unicode fonts remain incomplete. SVG
filter effects may become bitmaps. Hyperlink annotations, semantic document
tags, editable Samsung object structure and original pen channels are not
preserved as PDF features. The new PDF API is currently Rust/CLI functionality;
browser export requires a separate integration.

The APK's own vector list exporter rasterizes stroke batches before PDF
insertion, as documented in [native PDF stroke findings](native-pdf-stroke-findings.md).
Its output is not an all-vector ground truth for pen geometry. The SDK's
SVG-based architecture can preserve supported paths while pen appearance
and opacity are investigated independently.

Public export option names also differ from the native factory types.
[Standard PDF composition findings](standard-pdf-composition-findings.md)
trace the actual UI choice to its X delegate, ordered ordinary-object
batches and explicit Darken highlighter pass. Reference captures should
record the UI option instead of inferring the implementation from its name.
