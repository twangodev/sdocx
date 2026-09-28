# SVG generation

`render.rs` converts native objects into the `svg` crate's elements and path
commands. `ink` owns Samsung stroke reconstruction; serialization does not change
its pressure, tilt, movement, or sampling algorithms. Preview, replay, and PDF
conversion consume the same SVG renderer.

`render/vector.rs` provides shared composition and replay helpers:

- `Scene::scope` owns nested elements and serializes completed top-level subtrees
  so a dense page does not retain a full element tree.
- Typed gradient, mask, and clip references use one page-local ID allocator.
- `ReplayPath` and `polyline` derive sample offsets from serialized attribute
  values. Never compute replay offsets from XML length or unformatted numbers.
- `Inline` prevents formatting whitespace from entering rich text and links.
  User text and attributes pass through library escaping. The internal `Blob`
  holds only output already serialized by library nodes.

For a new native object, construct library elements in its render function and
compose them through `Scene`. Keep geometry reconstruction separate. Add shared
helpers only for recurring semantics; do not build XML snippets or a second SVG
object model.

Path commands use the library's `f32` coordinates. Existing export precision is
rounded before that conversion; transforms and other attributes can retain
`f64` values. Native shape paths outside the finite `f32` range are omitted as a
whole. Tests compare parsed attributes and path commands, not attribute order or
trailing zeroes. Replay, text whitespace, vector PDF shading, and Chromium
appearance have separate regression coverage.
