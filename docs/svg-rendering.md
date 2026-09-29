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
