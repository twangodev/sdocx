# Native TapePen pattern source control

The pinned APK pattern 1 SVG is loaded into the native pattern store. A direct Unicorn call to `SPen::RasterizeTapePenPatternFromSvg(1, width, height, vector)` returns the texture bytes before the Drawing or GL upload. At 64×64 and 128×128, respectively, these vectors have SHA-256 `78a0a70b418c59c460f8791d48d7f131ad7d445f3797323eb041fd9b689ba189` and `f290e56addf62df35c7fbd4bfb6bddaeccc047ab382c52aa9766cafb38b7d292`. These match the independently recorded `glTexSubImage2D` payloads in the 256- and 512-display controls exactly. All alpha bytes are 255. In the 128 output, the most frequent background and stroke colors are RGBA `(200,233,241,255)` and `(63,84,128,255)`; antialiasing produces intermediate opaque colors.

Run `UV_CACHE_DIR=/tmp/sdocx-uv-cache uv run --offline --with unicorn==2.1.4 python tools/tapepen-public-drawing/native-pattern-source.py --run --shape64 --size12`. The wrapper verifies the capture harness inputs and their preflight contract, then exits after checking both direct calls. It requires the same locally extracted, hash-pinned APK resource, native libraries and host dependencies as the Drawing captures. The output hash checks compare against the retained capture payload identities; captured pixels are not inputs to the native call.

This identifies a source-to-upload path for this asset at these sizes, under the capture's explicit host math and emulation adapters. It is a native test oracle, not a Rust, WASM or general SVG rasterizer. Two changed SVG inputs were subsequently checked at 64 and 128; see `capsule-scan-control-README.md`. Arbitrary dimensions, Android-device behavior and public export integration remain unverified.

This is a workspace probe, not a standalone checkout replay. It requires the
local `capture_candidate.py`, `preflight_candidate.py`, `tapepen_canvas_native.py`,
`preflight.json`, `candidate.json`, `phase-control.patch`, and `rotation-v113.patch`
in this directory, with the exact SHA-256 identities in the probe's `pins` table.
The capture further requires its pinned helpers under
`tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance`, the
watercolor bridge under `tmp/vector-parity-plan/pen-family-audit`, its lineage
scripts under `scratch`, the extracted libraries under
`scratch/apk-analysis-native/arm64-v8a`, AArch64 binutils under
`scratch/apk-analysis-runtime`, and the pattern asset under
`scratch/apk-output/samsung-notes-4.4.45.37/res/raw`. The local preflight manifest
and capture startup checks define the complete dependency identities; those
large capture harnesses and extracted inputs are not bundled with this probe.

The retained upload comparison evidence is
`identity-screen-state-256.log.gz` and `identity-screen-state-512.log.gz` in this
directory. The direct replay checks their previously recorded payload hashes;
it does not recapture the Drawing uploads. Successful execution prints one
`native pattern source` line for each dimension after checking its bytes and
opaque alpha.
