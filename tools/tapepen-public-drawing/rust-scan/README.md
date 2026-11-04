# TapePen pattern rasterizer evidence

The maintained `sdocx::tapepen_pattern::rasterize` API and WASM `rasterize_tapepen_pattern` accept caller-supplied source at sizes 64, 128 or 256 and return straight RGBA8. They recognize the observed 120×120 pattern structure, two full-canvas fills and 136 single-segment solid-color, round-cap paths; this is not a general SVG parser. Unsupported grammar, size, values and bounded geometry return errors. No APK asset or captured pixels are packaged, and this explicit texture API does not change document SVG or PDF rendering.

The standalone Rust prototypes and Python comparison wrappers have been retired. Their source parsing, bounded stroke geometry, scan conversion and blending checks are maintained in the crate. The SDK test `tapepen_pattern` checks source-mutation hashes, seven retained native output hashes, malformed inputs and the scaled-width cutoff. The original source must contain 136 occurrences of the controlled color and width. Width 0.015 is below the width cutoff at 64 and above it at 128; both upstream outputs nevertheless quantize to the source background color. Width 0.01874999888241291 reaches exactly the float32 cutoff at 64 and is skipped. Malformed color and nonfinite geometry still fail for skipped strokes. These additional cutoff cases were upstream controls, not native captures.

Run the native-hash controls with the pinned local source:

```sh
SDOCX_TAPEPEN_SOURCE=/absolute/path/to/tape_pattern_01.svg cargo test -p sdocx --test tapepen_pattern
```

Without `SDOCX_TAPEPEN_SOURCE`, that optional test returns early; only the synthetic and invalid-input tests execute. The source SHA and original/color/width native texture hashes remain in `texture-receipt.json`. Its prior producer references are historical provenance. These checks compare to retained host observations, not fresh native captures or device parity. The [256×256 source control](../v248-README.md) covers the original caller-supplied SVG; other source mutations at 256 remain unverified.

The retained Node verifier checks the same seven native output hashes and five rejected-input controls:

```sh
node tools/tapepen-public-drawing/rust-scan/verify_wasm.cjs /absolute/path/to/node-bindings/sdocx_wasm.js /absolute/path/to/tape_pattern_01.svg
```

It requires built `sdocx-wasm` and Node bindings generated with the matching wasm-bindgen CLI. It does not verify browser composition or device pixels.

Historical staged comparisons matched 816 upstream stroke coverage maps, 816 upstream edge lists, and all six original/color/width full textures at 64 and 128 on the recorded host. Those intermediate-stage comparisons are retired; native output hashes do not establish native intermediate-edge parity or general SVG support. The upstream C adapters and header hashes remain for evidence, with external NanoSVG headers and the locally extracted APK source described in [the capture control](../capsule-scan-control-README.md). Native acquisition is unchanged. Maintained translated crate sources retain the upstream notice and identify modifications.
