# Typed SVG generation

Native converters in `render.rs` and `render/fountain.rs` use the private Rust API
in `render/vector`. Only that adapter imports `svg`, names SVG attributes, or
formats their values. It covers the emitted elements and attributes;
it does not expose a generic attribute setter or raw XML constructor.

`ink` owns Samsung stroke reconstruction. Preview, replay, and PDF conversion
share the resulting SVG renderer. [Processing progress](progress.md) describes
the observer APIs used by browser loading and exports.

Pages own one `objects: Vec<PageObject>` tree, retaining stored order, container
boundaries, source offsets and render-layer selection inputs. `strokes()` and
`elements()` expose borrowed recursive views; their mutable counterparts edit
the same content.

Composition selects root objects into Base, Top and Masking passes, preserving
stored order within each pass. A selected container draws its children in place;
child render IDs and highlighter flags do not select new page passes. Document
body text precedes page-local objects. Replay borrows the parsed strokes accepted
by that same root selection and uses dense stroke indices in stored traversal
order, independently of the order in which the passes draw them.
Debugger hit targets use `composed_objects()` in paint order, so picking and
replay share the Rust root selection. Source records supply annotation bounds
and types without selecting or ordering the objects again.

Native root intersection filtering is not implemented.
Saved container rotation has already changed each child's placement and angle;
the native drawing branch applies no inherited parent transform. Existing leaf
placement transforms remain in their converters. See
[native selection findings](reverse-engineering/object-selection-findings.md).
Unknown root render IDs are omitted rather than interpreted as known layers.
Declared render IDs blocked by preceding unsupported fields remain `Unresolved`
and produce a diagnostic; they do not become Base objects. A top-layer stroke
still overrides that common selection value. Opaque container metadata is
reported while retaining its child sequence.
The shared SVG top batch uses Darken on light paper and Lighten on dark paper;
this follows page capture, while Samsung's Standard list PDF path uses Darken.
The SDK keeps one SVG composition policy for preview and vector PDF output.

Rust tests cover mixed-object archives, hidden subtrees, root pass selection,
source offsets and dense replay indices. Independent overlap-color expectations
check image/stroke and shape/stroke order, including identical normal/replay SVG
pixels. PDF regressions inspect paint order through form objects, selectable
text and the absence of image objects for vector-only scenes. These establish
the supported composition contracts. Full mixed-container pixel parity remains
unverified against Samsung captures.

## Adding a native converter

Construct typed elements and compose them through `Scene::push` and
`Scene::scope`. Geometry, stroke styles, transforms, text settings, and replay
metadata have named methods and types. For example:

```rust
scene.push(
    Line::new()
        .x1(start.x).y1(start.y)
        .x2(end.x).y2(end.y)
        .stroke(Paint::Solid(ColorValue::Rgb(color)))
        .stroke_width(decimal(width, 2))
        .line_cap(LineCap::Round),
);
```

- `vector.rs` owns element constructors, applicable attributes, containers, and
  distinct gradient/mask/clip reference types. IDs come from one page counter.
- `vector/values.rs` owns finite numbers, precision, paints, transforms, and
  enums for styles and metadata. Gradient stops accept colors, not paint-server
  references. Text, URLs, and embedded media remain content values.
- `vector/path.rs` owns path commands with fixed arities and boolean arc flags.
  It also derives replay offsets from the serialized path or point-list values.

## Validation and serialization

Non-finite numbers, negative dimensions/radii/stroke widths, out-of-range
opacity or stop offsets, and invalid colors omit the affected element. Invalid
containers skip their children. Invalid paths are omitted completely, including
replay paths; a valid prefix is never emitted as a partial drawing.

Path commands use the library's `f32` coordinates. Supported saved shape/line
M/L/Q/C/Z paths narrow directly from `f64`, without fixed decimal rounding;
paths outside the finite `f32` range are omitted as a whole. Straight-line
endpoints and supported pathless shape bounds, ellipse radii, polygon points,
rotation/pivots and shape/line outline widths use validated numbers without fixed
decimal rounding. Image-local bounds, rotation/pivots and
cropped viewport/viewBox use validated `f64` without fixed decimal rounding.
Inline image translation still uses four places; other routes keep their policies.

Completed top-level subtrees are serialized promptly to limit element-tree
memory. The internal `Blob` holds only output already serialized by library
nodes. Inline composition prevents formatting whitespace from entering rich
text and hyperlinks. Library escaping handles text and attribute content;
existing hyperlink scheme validation remains in the native text converter.

Type-7 shape linear/radial fill and outline paint uses user-space gradient
coordinates derived from the admitted native common frame. Saved paths carry
no extra element rotation; procedural primitives use an inverse gradient
transform to preserve native page coordinates. Derived stops use the native
first-ten limit, singleton duplication, copied endpoints and 16.16 offsets,
retaining duplicate order and separate color/alpha. The
[paint findings](reverse-engineering/shape-fill-findings.md#consequences-for-this-codebase)
describe admission and native evidence boundaries.

Tests compare parsed SVG semantics, preserved text, complete replay boundaries,
invalid-input handling, vector PDF shading, and Chromium appearance. Synthetic
gradient archives verify parsed Rust SVG and vector PDF transport; they are
not paired Samsung scenes, and the real corpus has no gradient witness.

## Render diagnostics

[`RenderedPage`](../crates/sdocx/src/render.rs) carries separate text,
embedded-object, shape/line geometry and fill/outline paint diagnostics from
its render attempt.
Geometry reasons cover malformed, unsupported or unrepresentable saved paths,
invalid checked geometry, unsupported pathless shape templates, unsupported line
types and missing required line paths. A geometry diagnostic identifies the
object UUID and optional payload offset in its backing page. Paint diagnostics
identify the object, optional payload offset and paint role,
covering unsupported paint, missing retained paint source, invalid consumed
stops, unsupported gradient frames and unrepresentable gradient geometry.
Rejected paint is omitted without a solid substitute; the other paint role and shape text may still render.
Neither diagnostic list covers every invisible object or omitted pixel.

The additive WASM `render_svg_detailed` and `render_pdf_pages_detailed` methods
return output and diagnostics together from one render/export. Existing
`render_svg`, `render_pdf` and `render_pdf_pages` methods retain their payload-only
results. The browser uses detailed results and validates their report shape and
selected visible-page order; it does not synthesize diagnostics or read a mutable
last-report field. PDF selections retain caller order and repeated pages.

`page_index` in detailed reports identifies the zero-based visible-layout page;
`source_page_index` identifies its backing parsed-document page. PDF report array
position is the output ordinal. Page-indexed PDF rendering errors use that output
ordinal; `InvalidPageIndex` instead identifies the requested visible-layout index.
These identities can differ after reflow or an ordered/repeated export selection.

Parser inspection, document preview, export-dialog page preview and completed
download reports have separate scopes. Preview results are guarded by document
and render generations; dialog results also expire when the selected page or
theme changes. Download reports use the export's captured theme and document
generation, rather than merging with current preview notices. Debugger
background/replay requests still return payload-only SVG. An empty report does
not certify full native appearance or preservation of every source field.

Public `RenderedPage`, `PdfOutput` and `PdfPageDiagnostics` are non-exhaustive.
Use their `new` constructors and read or update their public fields; destructuring
requires `..`. New render diagnostic categories may be added, so matches on
diagnostic kinds require a fallback arm. These boundaries let the SDK extend
reports without breaking callers. With `serde`, old
`RenderedPage` JSON may omit it and receives an empty list, matching the existing
geometry-field default; `PdfPageDiagnostics` supports serialization, not
deserialization. Detailed WASM reports and browser report validation require
the paint-diagnostic array. Legacy SVG strings and PDF byte results retain
their existing payload-only contracts.

## Text styling

Standalone boxes, document flow, shape text, table cells and code lines use
`render/text` to resolve local styles. `TextIndex` supplies checked character,
UTF-16 and byte boundaries to parsing, slicing and rendering. Native paragraph
ordinals count CR and LF separately, including empty trailing paragraphs; visual
line handling currently coalesces CRLF. Spans that split a surrogate pair are omitted
from rendered styling. Explicit boolean spans can disable an earlier style.
Font-name spans retain their native UTF-8 names. The typed family value uses
`cssparser` string serialization before SVG-library attribute escaping, so a
stored comma or quote cannot introduce additional font families or XML nodes.
Unknown names remain in the SVG and can use the declared generic fallback.

Text remains selectable SVG text, including spaces retained by wrapping.
The [text support table](text-vector-support.md) describes the shared Rust
layout engine, font resources, diagnostics and SVG/PDF transport limits.

Missing text styles use Samsung's `#262626` base color and logical font size 17,
then apply the current theme and coordinate conversion. Font-size conversion
uses the native minimum of one logical unit and preserves larger stored sizes.
Typed logical/resolved size units keep prepared child text from applying that
minimum, delta or density conversion again; resolved sizes below one remain
valid. Tiny-font SVG number serialization has not been audited for parity.
The document's native default dimensions and orientation supply density:
portrait width or landscape height divided by 360. Missing/nonpositive density
uses one. The stored body font-size delta applies before the logical minimum and
density conversion, including within local spans and headings. The device-default
`i32::MIN` sentinel is retained but currently resolves to zero delta; Samsung's
device configuration is unavailable. Margins and paragraph pixel spacing use
that same document density. Stored object coordinates remain unchanged.
See [native text layout inputs](reverse-engineering/text-layout-findings.md)
for scale, spacing, margins, gravity, markers and embedded-object contracts,
and [table findings](reverse-engineering/table-code-findings.md) for captured
preparation and painting rules.

Missing or malformed saved sections use validated full-source body reflow through
the same engine and all physical page boundaries. Balanced slices remain useful
for inspection; empty slices can still display measured source text on their
physical page. Removing or editing an inspection object invalidates this source
context. The runtime source snapshot is shared across pages and omitted from
serialization; rebuild deserialized layouts with `layout_document` to enable
canonical full-source reflow.
Body text uses the widest physical page in its selected measurement group,
matching the native body document's width producer. Each requested viewport
retains its own physical width and height.

Glyph coverage is resolved before measuring. Covered Latin clusters retain their
Rust positions when a neighboring cluster needs font or positioning fallback.
Tabs preserve their source character and four-space measured advance. Explicit
bidirectional overrides stay grouped for fallback painting. The selected fallback
face's actual weight and style reach typed SVG attributes and the PDF shaper;
ambiguous duplicate faces that cannot be selected by those properties are skipped.
Only painted faces are embedded. Fallback family order is a deterministic SDK
policy over the supplied font database; Samsung device order remains unverified.
