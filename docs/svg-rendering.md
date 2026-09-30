# Typed SVG generation

Native converters in `render.rs` and `render/fountain.rs` use the private Rust API
in `render/vector`. Only that adapter imports `svg`, names SVG attributes, or
formats their values. It covers the elements and attributes we currently emit;
it does not expose a generic attribute setter or raw XML constructor.

`ink` owns Samsung stroke reconstruction. Preview, replay, and PDF conversion
share the resulting SVG renderer.

Pages own one `objects: Vec<PageObject>` tree, retaining stored order, container
boundaries, source offsets and render-layer selection inputs. `strokes()` and
`elements()` expose borrowed recursive views; their mutable counterparts edit
the same content. Rust callers and serialized page consumers must migrate from
the former owned `strokes`/`elements` fields to this object tree.

Composition selects root objects into Base, Top and Masking passes, preserving
stored order within each pass. A selected container draws its children in place;
child render IDs and highlighter flags do not select new page passes. Document
body text precedes page-local objects. Replay borrows the parsed strokes accepted
by that same root selection and uses dense stroke indices in stored traversal
order, independently of the order in which the passes draw them.
Debugger hit targets use `composed_objects()` in paint order, so picking and
replay share the Rust root selection. Source records supply annotation bounds
and types without selecting or ordering the objects again.

Native root intersection filtering is not implemented. It requires per-object
selection bounds and partial-content tests, rather than a stored-bbox overlap.
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
the supported composition contracts; new Samsung captures are still needed for
full mixed-container pixel parity.

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

Text remains selectable SVG text. Font measurement, wrapping, paragraph layout
and embedded-object placement are still being consolidated; shared style
resolution alone does not establish native typography parity.
Wrapping retains spaces in selectable text. Hash-locked native text expectations
cover ordinary body and heading origins on the first four visible pages and
code lines on the last two pages. Tests retain a 0.25 SVG-unit tolerance;
fresh ordinary baseline comparisons are within 0.0001 units after converting
the PDF's actual viewport. These observations do not establish
native kerning, fallback, justification, or recomputed pagination parity.

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
Ordinary placed, shape, flow, table and code lines use their largest local font
size, with a default 1.35 spacing multiplier. Pixel line spacing adds scaled
pixels to that size; percentage spacing multiplies it. The shared native
baseline is line advance minus 0.35 times that font size. The body starts at
its scaled component margin and applies enabled paragraph spacing, without
adding flow-page padding. Embedded objects have separate height/margin rules.
Point and checkbox markers use typed vector artwork and an explicit
mobile/tablet/UWP display target shared by preview and export. Numbered markers
own a nested shared text layout using the first content span's resolved size
and the default sans face. Their measured reservation precedes wrapping; the
first line retains marker placement, including gravity. Fonts and diagnostics
share scoped registries, and painting registers retained marker faces for
embedding. Body flow uses the same layout loop with an explicit saved-slice
continuation policy. Typed capture windows preserve native paragraph and
overlap context for the next measurement adapter. Table-cell placement, numeric
text advance,
continued-object context and complete pagination still have measured gaps.
See [native text layout inputs](reverse-engineering/text-layout-findings.md)
for context-dependent scale, spacing, margins and gravity contracts.
