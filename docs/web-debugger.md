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

The debugger uses the existing viewer and its color-mode setting. Selection is a
lightweight SVG overlay on the cached page image, with no replay Canvas allocated
until playback or scrubbing begins. During partial replay, a stroke-free
background is requested lazily from the same renderer, and handwriting uses its
shared color and pressure-width calculation in a Canvas overlay. The original
page image and gesture cache remain mounted for immediate reuse after replay.
The covered SVG is hidden during replay so browser paint and hit-testing do not
reprocess its paths on every scrub.
This is an inspection preview, not a new Samsung-fidelity renderer: eraser history and unsupported pen effects are not
reconstructed. Source bounds need not match the exact painted outline, especially
for flowing or transformed non-stroke content. Normal viewer/export rendering is
unchanged.

## Resources and validation

The WASM session retains one compressed source copy and at most one decompressed
entry, under the existing 250 MiB input and 256 MiB entry limits. The previous
entry is released before expanding another. The debugger loads stroke data for
one replay page at a time; optional record metadata is requested on demand.
Replay caches are created lazily on the first partial replay and retained when
seeking to the end, until the page changes or the sidebar closes. `PageCanvas`
uses the existing viewer camera and scroll container to render only the visible
page region. Resolution levels follow zoom and device pixel density, rounded up
in square-root-of-two steps so nearby zoom levels can reuse cached ink. During a
zoom gesture the current surface follows the camera transform; it is rerendered
at the new resolution when the gesture settles.

The visible Canvas and its composition buffer are each limited to eight million
pixels and 4096 pixels per side (64 MiB combined). On extremely large/high-density
viewports this cap can lower the effective resolution. Ink tiles are at most
512 × 512 pixels, cropped to their painted bounds, with a separate 48 MiB LRU
budget. Each tile contains an ordered batch of 32 strokes; offscreen batches and
strokes are culled before drawing. Reverse seeks reuse these tiles, and panning
reuses tiles at the same resolution. `CanvasCache` also manages eviction and
backing-store cleanup for the existing gesture preview cache.

Cold seeks yield between batches after a 6 ms work budget and use the latest
requested position. One batch or unusually large stroke may exceed that budget.
Page and ink changes invalidate replay tiles; old resolution levels are evicted
within the same byte budget. Disposal clears all backing stores.
These limits are additional to the existing parsed document and browser rendering memory.

Changing or closing the file disposes the worker document, source buffers,
selection, URLs, and animations. Closing the sidebar releases its replay data,
background URL, and overlay surfaces while leaving the existing viewer images, camera, and gesture cache
intact. Opening the sidebar does not render a second static preview.

Unit tests cover timing fallbacks, byte bounds, exact integers, source identities,
and stale worker requests. Browser tests cover inspection, replay, narrow-screen
panels, and URL cleanup. The optional dense replay test uses
`SDOCX_DEBUG_FIXTURE` or `tmp/stroke-conformance/handwritten.sdocx` and records frame
cadence and Canvas memory without making machine-dependent frame-rate assertions.
The dense scrubbing test also measures backward seeks, checks raster reuse, and
compares pixels after revisiting a timestamp. Zoom tests cover 400% rendering at
2× screen density and scrolling through the visible region.
