# Processing progress

Decoding, layout, SVG composition and PDF export report work from the existing
Rust implementations. `Progress` contains a typed `ProgressStage`, completed
units and an optional total. Counts belong to the current stage; they are not
an estimate of elapsed time or overall document completion.

The SDK exposes additive observer APIs:

- `parse_bytes_detailed_with_progress(bytes, options, observer)` counts extracted
  media resources, physical pages and recursively visited objects in the active
  layer. Hidden subtrees count as processed when skipped.
- `layout_document_with_progress(document, observer)` counts visible pages.
- `DocumentTextCache::render_layout_page_svg_with_progress` counts composed
  objects, including children of selected containers, using the same root paint
  selection as ordinary SVG rendering.
- `render_layout_pages_pdf_detailed_with_cache_and_progress` reports scene
  composition and written PDF pages in the requested order, including duplicates.

Preparation and finalization have no quantified total. A completed object or
page counter does not mean that the entire operation has finished. Observers
execute synchronously; completion and errors are returned by the original
operation. Existing APIs remain available.

WASM adds `DocumentSession.create_with_progress`,
`render_svg_detailed_with_progress` and
`render_pdf_pages_detailed_with_progress`. Each accepts a JavaScript listener.
The bridge sends stage transitions, first completed units and stage completion
immediately, throttling other updates to 75 ms. Unknown totals serialize as
`null`. Listeners are observational; listener errors do not change vector
output. Recursive rendering on the same session returns a busy error.

The web worker forwards each update with its request ID, operation and document
generation. The client ignores stale or completed requests and routes progress
to the requesting operation. Import status, the viewer, export preview and
export download use the same accessible progress component. Unquantified file
reading, renderer startup, hashing, storage, image encoding and archive packaging
use indeterminate bars. Browser image loading is separate from SVG generation.
Cancellation terminates the worker, which also interrupts synchronous WASM work.
