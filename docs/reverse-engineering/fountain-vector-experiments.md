# Fountain vector rendering experiments

These are diagnostic prototypes, not production implementations. None changes
the Rust pen geometry or adds a rendering dependency. Current behavior and
validated native parity are described in [fountain-parity.md](fountain-parity.md).

## Firefox blend workarounds

Two vector-only workarounds were measured against the native cache's first
700 by 700 pixels. Expanding both the mask and its painted rectangle to the
viewport origin restores ink, but Firefox still has -18.90% alpha coverage
and 27.15% normalized absolute alpha error. Wrapping each stroke in a bounded
SVG pattern gives -19.32% coverage and 30.69% error; it also worsens Chromium's
error from 6.99% to 10.69%. Neither is a parity fix. The origin-expanded crop
took 8.4 seconds to decode/draw/read back in Firefox versus 0.45 seconds for
the incorrect original, and 3.6 seconds versus 0.65 seconds in Chromium
(single diagnostic runs, not benchmarks). Explicit sRGB interpolation does
not fix the pattern variant; linearRGB makes it worse.

An independent 64 by 64 overlap probe separates blending from antialiasing
and placement. Two radius-20 stamps at (32,32) and (40,32), with horizontal
gradients, overlap at pixel (24,32). The first stamp is fully opaque there,
so maximum coverage must remain 255. Chromium gives 255 with Lighten and
124 with normal blending; Firefox gives 125 with either mode. The browser
test `SVG fountain masks preserve maximum coverage at overlaps` records this
as a separate expected Firefox failure without requiring a local document.
Restoring nonzero coverage therefore cannot establish native V14 shading.

### Coverage-band experiment

A blend-free prototype represented each stamp's coverage threshold as a circle
clipped to a tangent-aligned strip. At threshold `a`, the strip half-width is
`1 - (a - 0.07) / 1.86` in unit-circle coordinates. Consistently wound subpaths
combine stamps into one nonzero-fill path per threshold without a bitmap or
polygonal circle approximation. Tests used 8, 16, 32, and 64 increments between
0.07 and 1, retaining exact production centers, radii, and directions.

This is not an acceptable replacement. Drawing nested translucent paths
accumulates antialiasing at their common boundaries. On the native cache's first
700 by 700 pixels, Chromium coverage increased by 10.41% at 8 increments and
13.18% at 64, with normalized alpha errors of 11.64% and 14.68%. Firefox had
similar broadening (+9.46% and +12.66% coverage). Painting opaque grayscale
thresholds inside an ordinary luminance mask also accumulates coverage at shared
edges: Chromium's alpha error rose from 16.76% to 29.73%, and Firefox's from
15.36% to 27.07%. More levels therefore do not converge to the native appearance
at this output scale.

The crop contains 5,691 stamps; serialized prototypes grew from about 6.5 MB
at 8 increments to 46.9 MB at 64. These experiments remain outside production.
Any replacement must resolve maximum coverage while preserving continuous
shading and edge coverage, rather than stacking antialiased threshold contours.

### Continuous-gradient partition experiment

A Shapely prototype partitions each stamp into its opaque middle and two
affine-gradient lobes. For intersecting lobes, their gradient difference is an
affine function, so a half-plane identifies which stamp contributes greater
coverage. Polygon differences remove dominated regions; SVG retains continuous
linear gradients and ordinary luminance masks, with no blend modes or filters.
This prototype approximates each circular boundary with 256 polygon edges;
it does not alter the source stamp centers, radii, or directions.

Partitioning against all stamps gives about 4.08% normalized alpha error in
Chromium and 4.07% in Firefox on the 700 by 700 native crop, with about -2%
coverage. However, this version is unsuitable: adjacent antialiased partitions
leave seams. Two radius-20 stamps at (32,32) and (40,32), with perpendicular
gradients, expose an interior error of 61 alpha levels against the analytical
maximum. The handwriting crop alone had hidden this problem.

Incremental partitioning instead draws each stamp only where its coverage is
at least the maximum of earlier stamps, retaining earlier ink underneath.
The middle and surviving lobes are united before drawing each gradient, so
their internal borders are not separately antialiased. The wide-stroke probe's
maximum interior error falls below one alpha level in both browsers. Each
stamp depends only on its predecessors, which also permits prefix replay.

On the native handwriting crop, incremental partitioning gives +1.22% coverage
and 8.75% alpha error in Chromium, and +1.14% coverage and 8.80% error in Firefox.
A 4x render reduced to the reference size gives -0.50% coverage / 3.13% error
and -0.52% / 3.14%, respectively. These scale-dependent checks diagnose edge
sampling; they do not introduce a supersampling requirement into production.
The prototype's 5,691 stamp regions serialize to 22.6 MB and take about 1.4
seconds to construct in Python on this host. Normal-scale decode/draw/readback
took 450 ms in Chromium and 356 ms in Firefox in single diagnostic runs.

Incremental partitioning remains an experiment. It restores continuous shading
in Firefox and resolves the analytical seam, but has larger output and higher
normal-scale native alpha error than the existing Chromium gradient renderer.
A Rust implementation would need bounded geometric approximation, robust
polygon operations, full-page/PDF checks, and measured replay cost before
replacing production shading.

Recovering consecutive sampled circle edges as SVG arcs reduces this crop from
22,568,631 to 4,319,196 bytes. Recovery requires both endpoints to lie on a known
stamp circle and the edge to be no longer than one original sample interval;
clipping lines remain lines. Runs split before closing a complete circle.
The wide-stroke interior check remains below one alpha level. Chromium's native
crop coverage becomes -0.10% with 8.36% alpha error, and Firefox's -0.40% with
8.46% error. Decode/draw/readback took about 180 ms and 177 ms in single runs.
Arc versus polygon rendering changes edge sampling (about 2.2% normalized alpha
difference), despite preserving the analytical interior check. The naive Python
circle-matching pass takes about 21 seconds, so its construction cost is not
acceptable for production.

An isolated Rust probe compiled polygon difference through `i_overlay` 9.0.0
and path serialization through `svgwriter` 0.1.1 for `wasm32-unknown-unknown`.
This proves basic target compatibility on the installed Rust 1.98 toolchain,
not performance, minimum-toolchain compatibility, or fountain correctness.
`i_overlay` declares Rust 1.88 as its minimum. Source inspection of `svgwriter`
also limits its suitability: attribute names have generated setters, but their
values share a generic `Value` trait rather than attribute-specific types;
ordinary floating-point attributes round to six decimal places; and the public
element API has no arbitrary `data-*` setter for replay metadata. Its raw-XML
escape hatch would reintroduce manual markup. No production dependency was added.

