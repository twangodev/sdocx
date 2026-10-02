# Typed SVG generation

Native converters in `render.rs` and `render/fountain.rs` use the private Rust API
in `render/vector`. Only that adapter imports `svg`, names SVG attributes, or
formats their values. It covers the emitted elements and attributes;
it does not expose a generic attribute setter or raw XML constructor.

`ink` owns Samsung stroke reconstruction. Preview, replay, and PDF conversion
share the resulting SVG renderer.

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

Path commands use the library's `f32` coordinates. Existing export precision is
rounded before conversion; transforms and other attributes retain `f64` values.
Native shape paths outside the finite `f32` range are omitted as a whole.

Completed top-level subtrees are serialized promptly to limit element-tree
memory. The internal `Blob` holds only output already serialized by library
nodes. Inline composition prevents formatting whitespace from entering rich
text and hyperlinks. Library escaping handles text and attribute content;
existing hyperlink scheme validation remains in the native text converter.

Tests compare parsed SVG semantics, preserved text, complete replay boundaries,
invalid-input handling, vector PDF shading, and Chromium appearance.

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
