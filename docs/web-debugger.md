# Web debugger

Open an SDOCX file and toggle **Debugger** in the top app bar. It opens a right
sidebar beside the existing document viewer (a right drawer on narrow screens).
The file stays on this device; decoding and byte access run in the existing WASM
worker. The viewer, toolbar, zoom camera, page URLs, and gesture cache stay mounted.

- **Tree:** browse document metadata and diagnostics, every stored page, its
  layers and nested objects, and ZIP entries. The entry/page filter accepts names
  and page IDs. Collections load on expansion and show additional rows in batches.
- **Properties:** expand decoded fields, including unknown masks and preserved
  data. Object identity is its archive entry and payload offset. Optional decoder
  errors remain local to that record. Non-stroke semantic elements are available
  under their stored page; raw media is available under its archive entry.
- **Hex / ASCII:** inspect 4 KiB windows of an uncompressed entry, or choose
  **Original file bytes** to examine the ZIP and appended end tag. Offsets are
  decimal in the input and hexadecimal in the byte display. Selecting an object
  jumps to its payload. Integers outside JavaScript's exact range and raw object
  timestamps are displayed as decimal strings.
- **Shared viewer:** click an object's source bounds or select it in the tree. The blue
  rectangle marks the selected source bounds; the red point marks a stroke sample.
  The sample list displays coordinates, pressure, raw timestamp, tilt, and
  orientation, and only mounts the visible rows.
- **Timeline:** play/pause, restart, step between strokes, scrub, or choose
  0.25–4× speed. Playback stays on the selected stored page and pauses when the
  page changes or the sidebar closes. The sidebar has Tree / Properties tabs.
  Scrolling or navigating the viewer selects the corresponding stored page;
  choosing a stored page in the sidebar navigates the viewer when that page has
  a visible layout. Omitted physical pages remain inspectable without creating
  another preview.

## Timing and rendering limits

Playback follows the stored traversal order of visible strokes in the current
layer. Hidden objects, inactive layers, and unsupported records remain available
in the tree. It is **not an undo/redo log or a reconstruction of edit history**.

For strokes marked as millisecond mode with complete, nondecreasing timestamps,
playback subtracts the first sample time. Other strokes use a synthetic 16 ms
sample interval. Every inter-stroke gap is a synthetic 150 ms. Equal timestamps,
empty strokes, and single-point strokes are retained. The original values are
never rewritten.

Selection uses an SVG overlay on the existing viewer. Partial replay lazily
requests an annotated page SVG from the shared Rust renderer. `ReplaySvg`
reveals stamp groups or serialized path prefixes using the prepared
`sample_ends`; the browser does not recalculate pen geometry, shading or opacity.
Non-stroke content and blend groups retain the renderer's paint order.

Timeline changes reuse the same SVG DOM. A page or color-mode change loads a new
SVG. The normal page image is hidden while partial replay is visible and returns
at the end of the timeline. The viewer's camera and gesture cache stay mounted.

Replay reconstructs saved geometry. Eraser history, live prediction, temporary
tips and unsupported pen effects are not reconstructed. Source bounds need not
match the painted outline, especially for flowing or transformed content.
Fountain mask and blending limitations in Firefox are recorded in
[vector parity](reverse-engineering/fountain-parity.md#vector-replay).

## Resources and validation

The WASM session retains one compressed source copy and at most one decompressed
entry, under the existing 250 MiB input and 256 MiB entry limits. The previous
entry is released before expanding another. The debugger loads stroke data for
one replay page at a time; optional record metadata is requested on demand.
The annotated SVG is retained when seeking to the end, until its page or color
mode changes or the sidebar closes. Replay allocates no Canvas or ink tiles.
Large documents still require memory for the SVG DOM and can make seeks costly;
reusing nodes does not establish a frame-rate guarantee.

Changing or closing the file disposes the worker document, source buffers,
selection, URLs, and animations. Closing the sidebar releases its replay data
and SVG DOM while leaving the viewer images, camera and gesture cache intact.
Stale worker and SVG responses are discarded. Opening the sidebar does not
render a second static preview.

Unit tests cover timing fallbacks, byte bounds, exact integers, source identities,
and stale worker requests. Browser tests cover inspection, replay, narrow-screen
panels, and URL cleanup. The optional dense replay test uses
`SDOCX_DEBUG_FIXTURE` or `tmp/stroke-conformance/handwritten.sdocx` and records frame
cadence and SVG node counts without making machine-dependent frame-rate
assertions. Dense scrubbing checks retained nodes, backward seeks, zero replay
canvases and pixels after revisiting a timestamp. Zoom tests cover 400% rendering
at 2× screen density and scrolling through the visible region. Interaction and
normal/replay equality checks alone do not establish native appearance parity.
