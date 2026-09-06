# Reverse-engineering knowledge base

This directory is the maintained Markdown memory for Samsung Notes SDOCX/WDoc
reverse engineering. It records conclusions that are backed by the Samsung
Notes APK, native serializers, or real compatibility fixtures.

## Documents

- [`file-format.md`](file-format.md) — authoritative archive and binary-format
  map: `note.note`, pages, layers, objects, frames, strokes, media, hashes and
  end tags.
- [`source-map.md`](source-map.md) — where each conclusion comes from in the
  decompiled Java and native libraries.
- [`fixture-validation.md`](fixture-validation.md) — historical measurements
  from three retired fixtures, preserving the evidence behind the format map.
- [`stroke-rendering-findings.md`](stroke-rendering-findings.md) — root cause of
  the stray top-right strokes and the exact packed-point layout.
- [`text-box-findings.md`](text-box-findings.md) — native standalone-text frames,
  bounded rich-text decoding, diagnostics, regressions and rendering limits.
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
  coverage comparison, size conversion, thin-stroke smoothing and alpha-call audit.
- [`marker2-sampling-findings.md`](marker2-sampling-findings.md) — quadratic
  distance approximation, stored-point replay and ordinary stroke completion.
- [`stroke-recording-findings.md`](stroke-recording-findings.md) — event-sample
  appends, repeated-coordinate taps, optional replacement and replay source reset.
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
- [`spi-alpha-residual-findings.md`](spi-alpha-residual-findings.md) — partial
  mode-3 alpha decoding, signed run tokens, coefficient scans and native checks.
- [`spi-alpha-payload-findings.md`](spi-alpha-payload-findings.md) — mode-3
  prediction fields, partition masks, marker updates and complete payload traces.
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
  bounded fixed data and structured document metadata.
- [`note-metadata-findings.md`](note-metadata-findings.md) — optional application,
  author, pen, voice, attachment and fixed-style fields with bounded records.
- [`table-code-findings.md`](table-code-findings.md) — native inheritance chains,
  table styles, per-edge borders, bounded row/cell data and the nonnumerical
  row-height field order.
- [`math-findings.md`](math-findings.md) — native math envelopes, embedded
  formula boundaries, angle modes and connected plot references.
- [`plot-findings.md`](plot-findings.md) — plot coordinates, graph expressions,
  substitutions, colors, widths and visibility.
- [`formula-findings.md`](formula-findings.md) — formula expressions, answers,
  embedded strokes, image references and label graphs.
- [`formula-rendering-findings.md`](formula-rendering-findings.md) — image/ink
  precedence, image placement, visible-stroke bounds and expression-type limits.
- [`parser-roadmap.md`](parser-roadmap.md) — implementation sequence and
  compatibility rules for the Rust parser.

## Maintenance rules

- Mark facts as confirmed, inferred or unresolved.
- Prefer declared record sizes, offsets and masks over fixture-specific magic
  numbers.
- Record the APK version and fixture set used for a conclusion.
- Keep unknown fields and object types round-trippable where practical.
- Do not commit the APK, decompiled sources or compatibility documents here.
  Store the test corpus externally and keep only measurements/expectations in
  the repository.
- Keep the knowledge base Markdown-only. One-off disassembly/audit programs can
  remain disposable local tooling; durable conclusions belong in these files.

## Sources and validation

- Samsung Notes APK: 4.4.45.37 (`arm64-v8a`/`armeabi-v7a`), SHA-256
  `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
- Historical audit: 7,182 stroke objects, 924,442 points, three layer hashes
  and three page hashes with zero structural/hash mismatches. The retired
  fixtures are identified by digest in [fixture validation](fixture-validation.md).
- Current corpus and test commands: [`conformance/README.md`](../../conformance/README.md).
  Historical audit totals do not describe current corpus coverage.
