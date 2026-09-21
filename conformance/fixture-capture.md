# Samsung reference fixture capture

Create each note in Samsung Notes and export the same saved note as both
`.sdocx` and PDF. Keep these files together and use a new fixture ID when
changing their contents. Record the Samsung Notes version, device/OS, page
size, font names and export settings in the external dataset's notes.

## First priority: native shapes and lines

Suggested pair: `02-shapes-lines.sdocx` and `02-shapes-lines.pdf`.

| Page | Content to capture | What it checks |
| --- | --- | --- |
| 1 | Rectangle, oval, triangle, right triangle and diamond; a rounded rectangle; a short label inside a shape. | Template IDs, bounds, fill, outline and embedded text. |
| 2 | Copies at 0°, 30° and 90°; distinct fill/outline colors; opaque and translucent fills; several outline widths. | Geometry versus drawn bounds, rotation, ARGB and line styles. |
| 3 | Horizontal, vertical and diagonal lines in both directions; elbow and curved lines; examples of dash styles and each available arrowhead. | Endpoint order, paths, dashes and arrow placement. |
| 4 | Overlapping shapes, text and strokes; a long label that wraps; an object near a page edge. | Draw order, text layout and clipping. |

Use native shape/line objects from the app's tools. Add short labels describing
the intended settings so comparisons remain understandable. If a setting is
unavailable in the app version, record that limitation instead of substituting
a screenshot or edited SDK output. Separate gradient or specialized-template
fixtures can follow once the basic pair is validated.

## Image placement and standalone text

- `03-image-placement`: repeat the same small source image across pages;
  include an uncropped placement, a crop, rotations and a border. Preserve the
  original image alongside the exports in the dataset.
- `04-standalone-text`: include empty/short boxes, Unicode, mixed styles,
  rotation, wrapping, margins/alignment and an overlapping image or shape.

## Validate and register

1. Reopen the saved source in Samsung Notes and inspect every PDF page for the
   intended content. Confirm that the PDF came from the same saved revision.
2. Store the pair in the `hf/` dataset submodule, following the corpus license
   and attribution conventions. Keep APKs, fonts and document binaries out of
   the reverse-engineering knowledge base.
3. Compute SHA-256 for both files. Inspect parsed structure and visible page
   counts before adding the matching entry to `corpus.json`; do not guess counts
   from ZIP entry order. Text expectations are optional. Add exact page-object
   counts and any confirmed diagnostic counts using the
   [manifest format](manifest-format.md).
4. Run structural conformance and the visual runner with the new ID. Preserve
   diagnostics, reference/SDK images, tool versions and font hashes. Add focused
   synthetic regressions for each confirmed parser or renderer defect.
5. Commit and push the dataset changes to Hugging Face, then commit the `hf`
   submodule pointer and manifest changes together in the SDK repository.

Successful parsing and similar pixel scores establish coverage of these cases;
broader compatibility still requires additional app versions and documents.

## Full bundled-pen parity captures

Capture each pen available in the installed Samsung Notes UI, keeping the
saved SDOCX and its matching PDF together. Include the exact app/device/OS
version and the pen/settings shown by the debugger. Do not substitute a
similarly named pen for one unavailable in that UI; record it as unavailable.
The native registry and alias mapping are in
[`pen-selection-findings.md`](../docs/reverse-engineering/pen-selection-findings.md).

For each pen, use a labeled page with three sizes and the following cases:

- Taps and very short strokes; slow circles, loops, tight turns and zigzags.
- Light-to-heavy and heavy-to-light pressure ramps, abrupt pressure changes,
  and strokes made at different speeds with comparable pressure.
- Upright and tilted stylus strokes, repeated directions, and visible starts
  and ends. Include any fixed-width option separately.
- Self-overlaps, overlaps between separate strokes, opaque/translucent ink,
  and light/dark backgrounds. For erasers or destination-dependent brushes,
  preserve the drawing order and include earlier ink beneath the effect.
- Each available texture, advanced setting and straight-line alias separately.

Use the existing 02 FountainPen calibration as the first reference, not as a
proxy for all pens. Registry recognition and implementation status are
separate from reference-validated rendering. Pens without matching captures
remain unverified; full bundled coverage must not be inferred from page-wide
pixel scores on the existing corpus.
