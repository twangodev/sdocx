# Vector retention boundaries

## Evidence and meaning of retention

These findings trace the current Rust parser, inspection APIs and vector output.
Native contracts are linked to their separate findings; no new native execution
or Samsung appearance comparison is claimed here.

| Representation | What it retains | What it does not establish |
| --- | --- | --- |
| Original archive or uncompressed entry bytes | Opaque records, encoded geometry and resource bytes | Semantic decoding or a renderer |
| `StoredPage` and `StoredObject` | Object types, byte ranges, integrity trailers and ordered children | Ownership of the bytes identified by those ranges |
| High-level `Document` | Supported selected objects, typed fields and some bounded opaque fragments | Every saved object, field, layer or resource |
| Generated SVG/PDF | Supported drawable geometry and text transport | Original binary identity or complete native appearance |

Retaining vectors includes their resource and placement identity. Original source
images remain images; retaining those bytes does not introduce rasterization.
Reconstructed vector pen geometry can preserve a drawable ink representation
without being a copy of an original saved outline.

## Original page bytes are external to the parsed model

[`StoredObject`](../../crates/sdocx/src/storage.rs) stores `payload_offset` and
`payload_size`. Its `payload(page_bytes)` method borrows the corresponding range
from a caller-supplied uncompressed `.page` entry. `ParsedDocument` owns the
physical indices, high-level document, typed note, manifests for page order and
diagnostics, but no original page-byte store.

The [archive parser](../../crates/sdocx/src/container.rs) reads each page into a
local buffer, decodes it, and returns only semantic values and physical indices.
The buffer does not survive in the returned `ParsedDocument`. Ordinary `parse`
also discards the physical indices and parse report. Consequently, an opaque
page-object payload remains recoverable only when the caller retains or reopens
the matching original input. Serializing offsets does not serialize their bytes.
The physical page-header projection also keeps only the low 32 bits of each
variable-length mask, alongside its byte count; it does not own the original
high mask bytes or page flexible data.

The browser has a separate ownership boundary: its
[`DocumentSession`](../../crates/sdocx-wasm/src/lib.rs) retains a
[debugger source](../../crates/sdocx-wasm/src/debugger.rs) containing original
archive bytes. This permits later structural inspection in that session. It
does not add source ownership to the standalone Rust `ParsedDocument` API.

## Selected objects and opaque parents

The [page decoder](../../crates/sdocx/src/page.rs) builds high-level objects from
the saved current physical layer. Hidden recognized objects and their subtrees
are omitted from that model. Inactive and hidden records still have physical
indices; explicit semantic inspection requires original page bytes. This agrees
with the recovered [physical-layer selection](page-layer-selection-findings.md).

[`PageObjectContent`](../../crates/sdocx/src/page_objects.rs) has stroke,
element and container variants, with no opaque object variant. Recognized
containers retain child boundaries and order, but do not retain their complete
common metadata. Unsupported parents are not represented as high-level nodes;
supported descendants are traversed separately and appended at that location.
This does not preserve the parent's semantic identity or establish its drawing
contract.

Saved type-4 container rotations are already applied to child geometry, as
[traced in native setters and drawing](object-selection-findings.md#saved-container-rotations-are-applied-to-children).
A second parent rotation would apply them again. That evidence does not establish
transforms or composition for opaque painting, brush or stroke-group objects.
The [support matrix](../rendering-support.md#object-types-and-locations) records
which native types currently have a dedicated semantic or rendering route.

## Ownership differs among supported object families

| Family | Owned semantic or opaque data | Remaining source dependency |
| --- | --- | --- |
| Regular stroke | Decoded point, pressure, time, tilt and orientation arrays; known style, tool and named properties; undecoded style remainder; resolved pen strings | Common metadata beyond bounds, raw stroke masks and additional frames require the original record |
| Shape and line | Common metadata, geometry, control points, saved path bytes, known outline enums, signed pen references and unsupported paint payloads | Magnetic points, connection blocks and some fixed/flexible/style extensions are discarded; shape/line pen IDs need the original note string table for resolution |
| Page image | Bounds, rotation, selected main/border/original references, crop and original placement; accepted asset bytes | Complete inherited shape/fill, borders, coedit span data, unknown fields and frames require original payload; negative media IDs collapse to absent |
| Rich text | Original style/paragraph payloads and ranges; full embedded WDoc object bytes; typed image/table/code projections | Enclosing common metadata, text-common extensions and page-textbox border/extra frames are not completely owned |
| Formula, math and plot inspection | Explicit metadata APIs retain mapped fields, masks and remainder bytes; formula strokes and math formula envelopes own their embedded binaries | The inspection call requires original page bytes; these values are not automatically attached to the high-level page model or rendered |

The concrete decoders are
[stroke](../../crates/sdocx/src/decode.rs),
[stroke metadata](../../crates/sdocx/src/stroke_metadata.rs),
[shape/line](../../crates/sdocx/src/shape.rs),
[image](../../crates/sdocx/src/image.rs),
[rich text](../../crates/sdocx/src/note.rs),
[formula](../../crates/sdocx/src/formula.rs), and
[math/plot](../../crates/sdocx/src/math.rs).

Rich-text embedded objects have stronger byte retention than physical page
objects: `RichTextObjectSpan.object_data` owns its whole embedded WDoc binary,
including when `content` is absent. The typed projection and that original binary
are different representations. Table-cell constructor-default spans also
represent native load state rather than the exact saved span count.

The formula [drawing contract](formula-rendering-findings.md#image-and-stroke-precedence)
uses a stored image when available and otherwise draws embedded source strokes,
then answer strokes. It does not evaluate LaTeX. Preserving those original stroke
records therefore does not require an expression evaluator. Image success and
stroke fallback must remain distinct to avoid drawing duplicate content.

Unsupported shape paint bytes are retained, including gradient payloads, while
[outline enums](shape-style-findings.md#saved-style-fields-and-loading) are typed.
This is stronger than their present drawing support. In contrast, decoded
[image effect settings](image-effects-findings.md), such as fill transparency and
nine-patch geometry, are not retained in `PlacedImage`. Keeping image pixels alone
does not preserve their clipping, transforms or effect placement.

## Resource identity and page paper

The [asset loader](../../crates/sdocx/src/container.rs) owns bytes for `media/`
entries ending in JPEG, PNG or WebP extensions. Other entries, including PDF
sources, are absent from `DocumentMetadata.media_assets`.

The [media manifest](../../crates/sdocx/src/media.rs) supplies authoritative
bindings while parsing. It is used by a transient resolver and is not returned
inside `ParsedDocument`; explicit manifest parsing is a separate API. A
`MediaAsset.archive_id` inferred from its filename prefix is not the same
identity as a manifest binding. A resolved array index also does not identify
immutable resource bytes when callers can replace assets.

The page decoder retains only the first PDF source-page index and discards
bindings, rectangles and additional entries. The
[PDF preservation constraints](../pdf-paper-preservation.md#current-rust-boundaries)
cover these losses, visible-page equality, original vector PDF transport and the
separate SVG conversion problem. Offset-only page retention applies equally to
non-PDF opaque vector records.

## Precision and drawable output

Saved WDoc points, rectangles and path bytes can contain `f64` values, while
[native geometry reconstruction](stroke-rendering-findings.md#shared-preparation-and-replay)
uses narrower arithmetic in specific drawing paths. Keeping source values and
performing native drawing arithmetic are separate operations.

The [SVG adapter](../../crates/sdocx/src/render/vector/path.rs) uses the library's
`f32` path parameters. Its `coordinate` helper first formats the requested
number of decimal places, then parses that value as `f32`.
[`native_svg_path`](../../crates/sdocx/src/render.rs) requests two decimal places
for saved shape/line paths. A `0.004`-unit displacement can consequently become
`0.00` before narrowing, even within the finite `f32` range. The owned native path
bytes remain unchanged. Other routes have different policies: basic shape/line
attributes and generic ink commonly use two places; stamp paths commonly use
four; some attributes accept validated `f64` values without fixed formatting.

This output quantization is an approximation boundary, not proof of an incorrect
native arithmetic port. Full saved numerical identity does not survive merely
because the emitted element is vector. The
[SVG serialization contract](../svg-rendering.md#validation-and-serialization)
already records these precision and validation rules.

Arc/oval native path commands retain their source bytes but reject the entire
unsupported drawable path. Invalid or unrepresentable SVG elements are omitted;
invalid containers skip their children. The adapter itself has no source-aware
omission report. Retained PDF glyph transport checks its own registry and reports
missing or unsupported retained text as export errors. That text safeguard does
not certify geometry omitted before SVG carrier parsing. Generic imported SVG
filters can also introduce rasterization; that separate compatibility output is
bounded in the [output support table](../rendering-support.md#output-and-verification).

## Diagnostic interpretation

Current parser messages describe unsupported known objects as “payload retained”
and unknown types as “retained unknown”. For physical page objects, this means
type and structural location are retained; it does not mean the returned model
owns the payload. Image/shape diagnostic categories likewise cover selective
semantic or opaque data rather than every source field.

An empty parse report is not a preservation certificate. Inactive objects are
not semantically decoded; regular stroke extra frames are validated and dropped
without a dedicated warning; some note/title/nested-text helpers discard their
unsupported-feature list; and invalid drawable elements may be omitted without
an output diagnostic. Conversely, an unsupported-feature warning does not mean
all original vector data was lost: embedded WDoc binaries, shape paths,
unsupported paints or retained source assets may still be available.
