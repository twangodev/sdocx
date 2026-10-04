# Reverse-engineering knowledge base

This directory records Samsung Notes SDOCX/WDoc findings from the APK, native
serializers and compatibility fixtures. Each finding identifies its source
and distinguishes recovered behavior from implementation and evidence limits.
Planning, task lists and development progress are kept outside these documents.

The [rendering support matrix](../rendering-support.md) maps current Rust
decoding, rendering and evidence boundaries across object types and features.

## Documents

- [`vector-retention-findings.md`](vector-retention-findings.md) — original
  byte ownership, semantic projection, opaque subtree/resource namespaces,
  native clipboard transformations and SPD migration, output precision and diagnostic
  boundaries in the current Rust pipeline.
- [`object-transform-findings.md`](object-transform-findings.md) — recursive
  container edits, hidden-child resizing, baked stroke geometry and precision.
- [`eraser-preservation-findings.md`](eraser-preservation-findings.md) — cut
  fragments, sample-channel propagation, removal and saved eraser distinctions.
- [`brush-record-findings.md`](brush-record-findings.md) — effective type-15
  dispatch, opaque brush/group admission and separate painting source/preview.
- [`painting-source-findings.md`](painting-source-findings.md) — editable `.spp`
  archives, ordinary stroke/page-string framing, packet membership and lazy-load/replay boundaries.
- [`shape-path-findings.md`](shape-path-findings.md) — saved path precision,
  route-specific curve normalization, drawing-route winding and native quadratic
  arc/oval construction.
- [`shape-fill-findings.md`](shape-fill-findings.md) — gradient/pattern records,
  drawing admission, vector paint geometry and separate PDF capabilities.
- [`connector-routing-findings.md`](connector-routing-findings.md) — saved line
  geometry, rotation, UUID attachments and edit-time routing dependencies.
- [`pdf-vector-transport-findings.md`](pdf-vector-transport-findings.md) — pinned
  PDF library resource preservation, page/catalog losses and import admission.

- [`pdf-paper-storage-findings.md`](pdf-paper-storage-findings.md) — saved and
  runtime PDF records, loader/JNI/setter boundaries and native binding predicates.
- [`pdf-paper-resource-findings.md`](pdf-paper-resource-findings.md) — manifest
  binding, synchronization identities, paths, availability and PDF open errors.
- [`pdf-paper-placement-findings.md`](pdf-paper-placement-findings.md) — list
  and continuous attachment geometry, inherited PDF boxes, views and clipping.
- [`pdf-paper-export-findings.md`](pdf-paper-export-findings.md) — source PDF
  imports, resource copying, glyph/Unicode boundaries and note overlay placement.

- [`rendering-corpus-findings.md`](rendering-corpus-findings.md) — observed
  feature occurrence and parse diagnostics across four locked pairs and three
  local research notes, with source identities and coverage limits.
- [`page-background-findings.md`](page-background-findings.md) — versioned
  PDF placement records, template/resource activation, native image modes and
  drawing routes, and current Rust retention limits.
- [`shape-style-findings.md`](shape-style-findings.md) — saved outline styles,
  dash patterns, compound passes, arrow geometry and native path conversion.
- [`image-effects-findings.md`](image-effects-findings.md) — image-fill field
  layout and defaults, crop/nine-patch selection, transparency, cache placement
  and legacy border drawing.
- [`card-source-findings.md`](card-source-findings.md) — Web, Link and attached-file
  source metadata/resources, regenerated previews and native PDF card export.

- [`marker4-v7.md`](marker4-v7.md) — fractional-width V7 highlighter vectors,
  native saved-redraw fixtures and isolated native-layer appearance comparison.

- [`file-format.md`](file-format.md) — authoritative archive and binary-format
  map: `note.note`, pages, layers, objects, frames, strokes, media, hashes and
  end tags.
- [`source-map.md`](source-map.md) — where each conclusion comes from in the
  decompiled Java and native libraries.
- [`fixture-validation.md`](fixture-validation.md) — historical measurements
  from three retired fixtures, preserving the evidence behind the format map.
- [`stroke-rendering-findings.md`](stroke-rendering-findings.md) — stray
  top-right stroke cause, packed-point layout, saved FountainPen V14/V16
  geometry and fallback rendering.
- [`fountain-parity.md`](fountain-parity.md) — native geometry coverage,
  Chromium/vector PDF appearance measurements and Firefox limitations.
- [`fountain-vector-experiments.md`](fountain-vector-experiments.md) — measured
  browser blend workarounds and unshipped vector prototypes.
- [`fountain-v14.md`](fountain-v14.md) — saved V14 redraw differences. The
  native oracle and Rust geometry port cover saved stylus input; directional
  shader coverage remains approximate.
- [`fountain-rainbow.md`](fountain-rainbow.md) — archived rainbow and V17
  research. Rainbow rendering is not implemented.
- [`fountain-rasterization.md`](fountain-rasterization.md) — stamp coverage
  and compositing research. Those shaders are not in the SDK.
- [`fountain-live-tip.md`](fountain-live-tip.md) — live tip state machine,
  separate from saved-stroke geometry.
- [`shapes-dot-calibration-findings.md`](shapes-dot-calibration-findings.md) —
  fixture 02 measurements, dot-background spacing and saved shape paths.
- [`text-box-findings.md`](text-box-findings.md) — native standalone-text frames,
  bounded rich-text decoding, diagnostics, regressions and rendering limits.
- [`text-layout-findings.md`](text-layout-findings.md) — native paragraph metrics,
  wrapping, embedded-object placement, vector text transport and shaping numeric
  domains, including Skia hinting, HarfBuzz advance conversion and captured
  post-shaping geometry, bounded Rust paint metrics and paint-sized shaping,
  mixed-script chunk arithmetic and itemization, native cache-word context,
  whole-piece Rust measurement and the bounded production paragraph adapter,
  logical-entry cache conversion, complete span paint profiles, native NAME/default
  selection and four-face measurement under supplied XML, captured consumer
  string metrics, captured ordinary
  block-selection/placement arithmetic, supplied-metric paragraph layout with
  host ICU, measured-line page range scans with actual Widget line/fallback
  producers and a typed Rust indexing kernel, supplied object-feedback/obstacle
  kernels and horizontal
  GPOS/fused-skew traces, with separate paragraph/vector
  transport boundaries and private physical-face SVG/usvg/PDF identities.
- [`text-draw-identity-findings.md`](text-draw-identity-findings.md) — native span
  measurement/draw identity, font metadata, UTF-16 glyph ownership, embedded-object
  background geometry, producer/emitter/export-caller boundaries, PDF alpha
  transport, cached-entry snapshots, XML font language, file-font source instances,
  live four-file registry source/language/copy identity, dynamic locale/custom
  fallback lifetime, certified whole-source
  paint plans shared by SVG/PDF and Chromium text clip
  behavior.
- [`image-findings.md`](image-findings.md) — displayed-image versus border/original
  references, authoritative media bindings and image regression coverage.
- [`shape-line-findings.md`](shape-line-findings.md) — native geometry and effects,
  pen references, bounded paths, rendering coverage and remaining fidelity gaps.
- [`visual-conformance-findings.md`](visual-conformance-findings.md) — measured
  Samsung PDF comparison, explicit PNG fonts and remaining visual gaps.
- [`pdf-export-findings.md`](pdf-export-findings.md) — shared SVG-to-PDF export,
  page units, embedded text/fonts and measured PDF validation.
- [`native-pdf-stroke-findings.md`](native-pdf-stroke-findings.md) — native
  vector-export stroke bitmaps, PDF image handoff and separate opacity inputs.
- [`standard-pdf-composition-findings.md`](standard-pdf-composition-findings.md)
  — public export option selection, Standard paint order, Darken highlighters
  and final object-batch flushing.
- [`end-tag-findings.md`](end-tag-findings.md) — native metadata boundaries,
  appended trailer precedence, bounded decoding and synthetic regressions.
- [`layer-findings.md`](layer-findings.md) — native layer identity, alpha-lock
  and shadow fields, bounded decoding and the Java transparency discrepancy.
- [`page-layer-selection-findings.md`](page-layer-selection-findings.md) —
  saved current-layer assignment, Standard PDF page pointers and semantic
  selection with complete structural retention.
- [`object-base-findings.md`](object-base-findings.md) — shared object visibility,
  editing flags, replay/resize values and preserved frame extensions.
- [`object-flexible-findings.md`](object-flexible-findings.md) — optional common
  fields, bundle boundaries and the distinct static extraction format.
- [`object-drawing-findings.md`](object-drawing-findings.md) — common visibility,
  container traversal, replay-order assignment and layer collection boundaries.
- [`object-order-findings.md`](object-order-findings.md) — file-order insertion,
  nested container order, stroke-only top selection and grouping boundaries.
- [`capture-composition-findings.md`](capture-composition-findings.md) — base,
  top and masking passes, object layer filters and capture clone state.
- [`stroke-metadata-findings.md`](stroke-metadata-findings.md) — stroke property
  polarity, ARGB colors, pen settings and legacy partial-rectangle records.
- [`pen-opacity-findings.md`](pen-opacity-findings.md) — pen-specific fixed
  opacity dispatch, theme-preserved alpha and Marker2 mask/composite equations.
- [`pen-selection-findings.md`](pen-selection-findings.md) — corrected stroke
  string IDs, native pen registry, fallback lookup and Marker2 version selection.
- [`marker2-rendering-findings.md`](marker2-rendering-findings.md) — V1/V2
  coverage comparison, size conversion, thin-stroke smoothing, and the saved
  Marker2 geometry used by the renderer.
- [`marker4-rendering-findings.md`](marker4-rendering-findings.md) — saved V8
  rounded rectangular tips, opacity, and fixture 04 before/after measurements.
- [`marker2-sampling-findings.md`](marker2-sampling-findings.md) — quadratic
  distance approximation, stored-point replay and ordinary stroke completion.
- [`stroke-recording-findings.md`](stroke-recording-findings.md) — event-sample
  appends, generated straight-stroke channels and separate shape identity,
  repeated-coordinate taps, optional replacement and replay source reset.
- [`motion-event-adapter-findings.md`](motion-event-adapter-findings.md) — Android
  sample channels, pointer-major history, raw coordinates and time origins.
- [`stroke-input-findings.md`](stroke-input-findings.md) — InkPen2 input-filter
  selection, raster recorder bindings and long-gesture splitting.
- [`inkpen2-input-findings.md`](inkpen2-input-findings.md) — beautifier sample
  admission, millisecond ordering, pressure cap and result/fallback routing.
- [`inkpen2-prediction-findings.md`](inkpen2-prediction-findings.md) — linear
  coordinate fitting, adaptive horizon, distance limits and retained timestamps.
- [`inkpen2-result-findings.md`](inkpen2-result-findings.md) — current/history
  distance checks, resampled-state rewriting and candidate-buffer lifetime.
- [`inkpen2-kalman-findings.md`](inkpen2-kalman-findings.md) — channel masks,
  exact noise constants, down reset and independent X/Y correction equations.
- [`stroke-prediction-findings.md`](stroke-prediction-findings.md) — real-event
  dispatch, separate Marker2 V2 prediction drawing and input-source mutation.
- [`prediction-length-findings.md`](prediction-length-findings.md) — prediction
  sample prefixes, gradual index-budget updates and the InkPen2 reset exception.
- [`uniform-latency-findings.md`](uniform-latency-findings.md) — callback timing,
  time-fraction cutoffs, timestamp interpolation and exact-boundary behavior.
- [`presentation-time-findings.md`](presentation-time-findings.md) — display
  orientation, hardware configuration and screen-position prediction delays.
- [`predictor-callback-findings.md`](predictor-callback-findings.md) — bundled
  predictor selection, callback registration, thread dispatch and event lifetime.
- [`predictor-queue-findings.md`](predictor-queue-findings.md) — main-looper
  delivery, Handler registry keys, callback cleanup and teardown boundaries.
- [`writing-view-teardown-findings.md`](writing-view-teardown-findings.md) —
  Java close order, native raster ownership and separate Handler cancellation.
- [`composer-close-findings.md`](composer-close-findings.md) — main-editor
  release order, Composer ownership, capture callbacks and save preparation.
- [`editor-release-preparation-findings.md`](editor-release-preparation-findings.md)
  — first-draw capture callbacks, document detachment and initialization posts.
- [`save-preparation-cancellation-findings.md`](save-preparation-cancellation-findings.md)
  — mode-gated shape cancellation, recognition flags and gesture-unlock callbacks.
- [`document-image-cache-findings.md`](document-image-cache-findings.md)
  — bitmap-save waits, SPI cache filenames and page canvas-cache associations.
- [`spi-media-findings.md`](spi-media-findings.md) — Maetel codec dispatch,
  length-prefixed media blocks and native decoder entry points.
- [`spi-header-findings.md`](spi-header-findings.md) — header layout checked
  with native routines, dimensions, color indices and packet acceptance.
- [`spi-data-packet-findings.md`](spi-data-packet-findings.md) — packed data
  prefixes, block-row groups, buffer reuse and native boundary checks.
- [`spi-codec-validation.md`](spi-codec-validation.md) — complete native
  bitmap round trips, block-mode coverage, alpha and output-capacity limits.
- [`spi-literal-block-findings.md`](spi-literal-block-findings.md) — mode-5
  plane layout, independent reconstruction, packet groups and alpha ordering.
- [`spi-copy-block-findings.md`](spi-copy-block-findings.md) — mode-0/1
  frame copies, displacement codes and independent mixed-block validation.
- [`spi-palette-block-findings.md`](spi-palette-block-findings.md) — mode-4
  palettes, index runs, packet cache state and original-image reconstruction.
- [`spi-differential-block-findings.md`](spi-differential-block-findings.md)
  — mode-2 symbol runs, row prediction, reduced planes and native boundary behavior.
- [`spi-color-intra-findings.md`](spi-color-intra-findings.md) — mode-3 color
  planes with zero quantization, signed prediction and reversible color conversion.
- [`spi-quantized-color-findings.md`](spi-quantized-color-findings.md) — nonzero
  quantization, coefficient escapes, integer inverse transforms and color clipping.
- [`spi-reduced-color-findings.md`](spi-reduced-color-findings.md) — mode-3
  reduced secondary planes, shared masks, mapped quantization and byte reconstruction.
- [`spi-temporal-block-findings.md`](spi-temporal-block-findings.md) — retained
  reference images, motion prediction and mode-3 submode-0/2 sequence decoding.
- [`spi-temporal-residual-findings.md`](spi-temporal-residual-findings.md) — temporal
  mask banks, side-specific coefficient tokens, scans and native bounds.
- [`spi-reduced-temporal-findings.md`](spi-reduced-temporal-findings.md) — bundled
  temporal residuals, secondary reference filtering and Q-zero transforms.
- [`spi-selected-plane-findings.md`](spi-selected-plane-findings.md) — submode-3
  plane updates, signed quantizer adjustment and Q-zero flag behavior.
- [`spi-reference-cache-findings.md`](spi-reference-cache-findings.md) — per-block
  cache ranks, shared fill counts, temporal copy syntax and alpha overlap behavior.
- [`spi-mixed-prediction-findings.md`](spi-mixed-prediction-findings.md) — all
  binary availability masks, edge completion and mixed intra/temporal sequences.
- [`spi-alpha-residual-findings.md`](spi-alpha-residual-findings.md) — partial
  mode-3 alpha decoding, signed run tokens, coefficient scans and native checks.
- [`spi-alpha-payload-findings.md`](spi-alpha-payload-findings.md) — mode-3
  prediction fields, partition masks, marker updates and complete payload traces.
- [`spi-alpha-pixel-findings.md`](spi-alpha-pixel-findings.md) — alpha edge
  selection, prediction equations, residual accumulation and pixel comparisons.
- [`spi-alpha-state-findings.md`](spi-alpha-state-findings.md) — neighbor
  initialization, packet boundaries, independent images and literal marker writes.
- [`spi-alpha-literal-state-findings.md`](spi-alpha-literal-state-findings.md)
  — literal marker offsets, bounded mixed sequences and allocation-dependent output.
- [`predictor-timing-findings.md`](predictor-timing-findings.md) — real-event,
  clock, VSync and refresh-period sources in external prediction callbacks.
- [`vsync-delivery-findings.md`](vsync-delivery-findings.md) — Java frame-time
  forwarding, native receiver subscriptions and neural predictor lifecycle.
- [`neural-model-findings.md`](neural-model-findings.md) — bundled M16/M20/M22
  selection, input gates, prediction horizons and filter configuration.
- [`neural-feature-findings.md`](neural-feature-findings.md) — rotated sample
  differences, timestamp gates, DPI scaling and model input-buffer order.
- [`neural-inference-setup-findings.md`](neural-inference-setup-findings.md) —
  requested tensor shapes, signature/interpreter setup and time-feature limits.
- [`neural-lifecycle-findings.md`](neural-lifecycle-findings.md) — runtime
  replacement, failure state, runner ownership and pending-task bindings.
- [`neural-output-findings.md`](neural-output-findings.md) — output coordinate
  scaling, inverse rotation, copied pen channels and independent timestamp fields.
- [`neural-selection-findings.md`](neural-selection-findings.md) — whole-ms
  horizon selection, candidate marking and callback current/history construction.
- [`neural-admission-findings.md`](neural-admission-findings.md) — acceleration
  gates, discarded output prefixes, expiry budgets and unbuffered bypasses.
- [`predictor-acceleration-findings.md`](predictor-acceleration-findings.md) —
  sampled motion history, cached contributions, weighting and integer angles.
- [`predictor-speed-findings.md`](predictor-speed-findings.md) — interval-speed
  averaging, endpoint history windows and Composer's low-speed threshold.
- [`predictor-chrono-findings.md`](predictor-chrono-findings.md) — time/VSync
  task pacing, phase thresholds and completion-dependent timer resets.
- [`predictor-dispatch-findings.md`](predictor-dispatch-findings.md) — base
  branch conditions, history-driven completion and separate Boolean returns.
- [`predictor-worker-findings.md`](predictor-worker-findings.md) — inline/worker
  routing, pending-task ownership, wait predicates and delayed input capture.
- [`predictor-reconfiguration-findings.md`](predictor-reconfiguration-findings.md) —
  instance recreation, enable-state preservation and presenter teardown order.
- [`predictor-device-policy-findings.md`](predictor-device-policy-findings.md) —
  model-prefix and SDK checks controlling worker construction and proxy kind.
- [`predictor-position-findings.md`](predictor-position-findings.md) — last-history
  presentation delays, coefficient arithmetic and backend-switch ordering.
- [`unbuffered-draw-findings.md`](unbuffered-draw-findings.md) — separate drawing
  cadence, receiver state, due checks and post-drawing reset points.
- [`neural-motion-findings.md`](neural-motion-findings.md) — minimum movement,
  real/output speed statistics and per-candidate distance limits.
- [`stroke-finalization-findings.md`](stroke-finalization-findings.md) — disabled
  constructor default, optional CSAPS processing and count-preserving replacement.
- [`stroke-insertion-findings.md`](stroke-insertion-findings.md) — first-point
  page selection, page-local translation and millisecond flags during insertion.
- [`view-input-transform-findings.md`](view-input-transform-findings.md) — child
  view conversion, event-history transforms, float precision and pen-width source.
- [`zoom-scale-findings.md`](zoom-scale-findings.md) — contents-view scale and
  scroll configuration, axis stretch and separate cutter/eraser scale dispatch.
- [`pen-size-findings.md`](pen-size-findings.md) — document-relative and density
  size levels, Marker2 bounds, native settings and recording-pen size copies.
- [`integrity-findings.md`](integrity-findings.md) — optional hash verification,
  exact coverage, unavailable checks and independent synthetic reference hashes.
- [`note-header-findings.md`](note-header-findings.md) — variable note masks,
  bounded fixed data, native admission/version authority and structured metadata.
- [`note-metadata-findings.md`](note-metadata-findings.md) — optional application,
  author, pen, voice, attachment and fixed-style fields with bounded records.
- [`table-code-findings.md`](table-code-findings.md) — native inheritance chains,
  bounded table/code records, merge construction, frame ownership and paint
  visibility; captured borders, fills, column minima, saved height limits,
  live child/image measurement and resize, ordinary one-page zero-height
  measurement and native page-padding production, actual cold/warm column-width
  and source-change
  producers with opaque host UText, first-pair padding capacity and actual cloned
  cell writer clip inputs and a bounded public Rust ordinary-page clip certificate,
  genuine document object feedback and Bodytext
  source-bound placement callbacks,
  cold/warm measurement, bounds, split caches, row-bottom compression, warm-row
  control, cell text dimensions, public layout lifecycle, visible rectangles and
  canvas clip arguments, PDF artwork crops, conditional text clips, independent
  cell/content Model rectangle setters and final native clip-path geometry, retained
  text entry/run bounds, complete cached-glyph run emission, actual Model drawn
  bounds, complete cell artwork,
  constructed cell Model bridges, genuine cell text constructors and native
  text-to-measurement/layout/cache producers, parsed Common/span defaults,
  captured pre-emission spans/maps, source getters with explicit normalization
  side effects and newline flushing, installed
  live cold/warm table layout with real child text, callback/span lookup, cloned table
  placement, measured/cropped background Canvas/image transport, code
  chrome/cache/minimum geometry and vector
  evidence limits.
- [`math-findings.md`](math-findings.md) — native math envelopes, embedded
  formula boundaries, angle modes and connected plot references.
- [`plot-findings.md`](plot-findings.md) — saved expressions/styles, mathematical
  viewport and derived segment/precision/cache boundaries.
- [`formula-findings.md`](formula-findings.md) — expression/answer records, native
  byte/JNI projection, embedded strokes, image references and recognition graphs.
- [`formula-rendering-findings.md`](formula-rendering-findings.md) — image/ink
  precedence, image placement, visible-stroke bounds and expression-type limits.
- [`parser-findings.md`](parser-findings.md) — structural decoding, metadata,
  compatibility rules and evidence limits for the Rust parser.

## Sources and validation

- Samsung Notes APK: 4.4.45.37 (`arm64-v8a`/`armeabi-v7a`), SHA-256
  `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
- Historical audit: 7,182 stroke objects, 924,442 points, three layer hashes
  and three page hashes with zero structural/hash mismatches. The retired
  fixtures are identified by digest in [fixture validation](fixture-validation.md).
- Current corpus and test commands: [`conformance/README.md`](../../conformance/README.md).
  Historical audit totals do not describe current corpus coverage.
