# Samsung PDF comparison and font fidelity

## Evidence and scope

The first measured visual pass uses the locked `01-basic-formatting` pair in
[`conformance/corpus.json`](../../conformance/corpus.json). Both source hashes
were verified before rendering. The document has six stored records and five
visible pages; its Samsung PDF has five 600 × 848 point pages. The SDK renders
1080 × 1527 pixel pages. Reference rasterization uses those pixel dimensions,
allowing the subpixel aspect-ratio difference caused by integer rounding.
Content is not shifted or aligned during comparison.

The [published corpus](https://huggingface.co/datasets/twangodev/sdocx-compatibility/tree/main)
contained only this pair when checked on 2026-09-04. It exercises flowing
text, formatting, Unicode, lists, a table and a code block. It provides no
standalone shape/line or image-placement visual coverage. The current
[locked manifest](../../conformance/corpus.json) also includes shape/dot,
text-flow image and Marker4 reference pairs; those do not change the scope of
the historical measurements below.

The reusable runner is [`conformance/visual.py`](../../conformance/visual.py).
It produces per-page PNGs, an interactive HTML report, CLI diagnostics and
JSON measurements with executable, source-file and font hashes. See the
[runner instructions](../../conformance/README.md#visual-comparison).

## Confirmed font mismatch

The Samsung PDF embeds a face named `Roboto-Regular`. The CLI previously
offered only host font discovery. Its generic fallback on this machine has
wider text than the reference: the first heading's dark pixels span x=50–787
instead of the reference's x=51–715. Wrapping follows the SDK's measured Roboto
advances, so a wider raster font can also extend text past the intended margin.

`--font PATH` supplies explicit faces for SVG, PNG and PDF export and can be
repeated. Supplied faces load before the pinned bundled faces; host fonts are
not discovered. All pages share the loaded font database. Missing or invalid
explicit files fail before any page output. SVG embeds the selected faces as
data fonts so its appearance does not depend on host font installation.

The measured run supplied regular and italic TrueType files from Google's
[Roboto source repository](https://github.com/googlefonts/roboto-2/tree/main/src/hinted).
These were kept in ignored local storage, with these SHA-256 digests:

| Font | SHA-256 |
| --- | --- |
| `Roboto-Regular.ttf` | `56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d` |
| `Roboto-Italic.ttf` | `fa0b17bb4aaac4a1b2ee149dd4ca3b55e97d3077aa6ba9bb02541b316e7c46ce` |

Directly extracting the PDF's font did not provide a usable SVG font: both
Unicode cmap tables in that embedded font were empty. PDF CID mappings can
still render it inside the original document. The family name alone is not
enough to establish that an extracted font can render Unicode SVG text.

## Historical raster measurements

Tools: Python 3.14.3, PyMuPDF 1.28.2, Pillow 12.1.1, NumPy 2.4.2 and the
CLI's resvg 0.47.0. Both runs used the same system font inventory. The only
rendering treatment change was supplying regular/italic Roboto; a subsequent
run without supplied fonts matched all five pre-change PNG hashes.

Changed pixels have an absolute difference above 16 in any RGB channel.
Missing ink is reference foreground with no SDK foreground within one pixel;
it includes displacement and glyph-shape differences, not just omitted text.
Foreground detection assumes the fixture's near-white canvas.

| Visible page | Changed pixels, system fonts | Changed pixels, explicit Roboto | Missing ink, explicit Roboto |
| ---: | ---: | ---: | ---: |
| 1 | 10.73% | 4.92% | 7.77% |
| 2 | 8.26% | 2.29% | 1.54% |
| 3 | 7.94% | 4.32% | 23.11% |
| 4 | 5.33% | 2.94% | 9.08% |
| 5 | 5.38% | 1.94% | 0.60% |

The five-page mean changed-pixel fraction falls from 7.53% to 3.28%.
Normalized mean absolute RGB error falls from 0.04568 to 0.01365, a 70.1%
relative reduction. This measures one fixture and font treatment, not overall
SDK compatibility or an automatic improvement to every host's default output.

Visual inspection confirms closer text widths. Page 3's Unicode samples retain
the highest unmatched reference-ink fraction. CJK/emoji font coverage, text
decorations, list markers and small baseline differences remain visible gaps.
The report also shows why SVG text extraction alone is insufficient: code-block
titles and lines remain in the continuation SVG at negative coordinates and
are correctly clipped. They are not duplicated on the visible page.

## Validation and evidence limits

The raster scores above describe the initial font experiment. Current Rust
PDF-coordinate comparisons check body/heading, numbered-marker, table-cell,
code and post-code text origins across five physical pages against the locked
native reference. Baseline tolerance is 0.001 SVG units. The retained
logical-canvas measurements in
[`conformance/text-metrics.json`](../../conformance/text-metrics.json) are
0.6 units higher than the actual viewport: 848.333333 versus 848 PDF points,
multiplied by 1.8. Numeric-item text X retains an approximately -0.04393-unit
font/advance difference. Point and checkbox artwork is typed vector geometry.
These comparisons do not establish complete pagination, font fallback or
whole-document visual parity. See
[native text layout inputs](text-layout-findings.md) for source addresses and
the [reference regression](../../crates/sdocx/tests/font_metrics.rs) for assertions.

Runner regressions cover blank-output detection, alpha compositing, tolerance,
rotated PDF dimensions, hashes, path boundaries, page ordering/counts, stale
output and font forwarding. CLI regressions cover supplied-font precedence,
repeated font options and missing/invalid font failures. Those tooling contracts
do not establish appearance parity for unsupported templates, arrowheads,
gradients or image effects.
