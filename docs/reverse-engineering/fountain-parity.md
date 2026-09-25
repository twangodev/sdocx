# Fountain pen vector parity

The target is Rust-generated vector SVG/PDF output matching Samsung Notes
4.4.45.37 fountain ink. This work is incomplete.

## Current implementation

The saved V14 reconstruction retains SmPath's direction vectors, including
its small-vector normalization threshold, zero-vector substitution, and the
final stamp's retained direction. Native fixture tests compare positions,
radii, directions and original-sample boundaries.

SVG/PDF output uses vector paths. Fountain shading is still approximated with
opaque circle coverage; retaining native directions does not yet fix that
appearance. The regression test
`fountain_svg_keeps_reconstructed_ink_as_vector_paths` verifies that a supported
native profile emits paths without embedded images.

The added Rust software rasterizer, WASM pixel API, fountain replay adapter,
and shader-comparison tooling have been removed. Existing replay returns to
its previous canvas path drawing. It is not a separate native shading solution.

## Verified geometry and representation

On 2026-09-25, the available `handwritten.sdocx` contained 2,769 V14 strokes and
56,713 stamps. The native saved-redraw oracle and Rust reconstruction agreed
exactly on x, y, radius, direction x/y and original-sample boundaries.

Its SVG contained 2,769 paths and zero images. Its PDF contained 2,770 vector
drawing objects, including the background, and zero images. These checks
establish geometry and representation, not complete visual parity.

Run the native geometry fixtures and vector regression from the repository root:

```sh
cargo test --offline -p sdocx --all-features --lib fountain
```

The optional hash-pinned native geometry oracles are documented in the
[conformance guide](../../conformance/README.md#native-geometry-checks).

## Remaining vector work

- Reproduce fountain appearance in scalable SVG/PDF using Rust-generated vector
  geometry and, where necessary, vector shading. Investigate the strokes that
  appear too wide without applying a global width adjustment.
- Validate stroke joins, endpoints, directional shading, pressure, tilt and
  movement against the decompiled behavior and Samsung reference exports.
- Check colored/translucent ink and page compositing while preserving vectors.
- Characterize saved profiles that still use approximations, including fixed
  width, non-stylus input, historical versions and effects.

Saved redraw evidence does not establish live prediction or temporary-tip parity.
