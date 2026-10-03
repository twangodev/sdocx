# Vector retention boundaries

## Evidence and meaning of retention

These findings trace the current Rust parser, inspection APIs and vector output.
The native opaque-record section concerns Samsung Notes 4.4.45.37 ARM64, APK
SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Native addresses are ELF virtual addresses, before relocation. Other native
contracts are linked to their separate findings.

| Native library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |

Native opaque-record admission, hierarchy and resource findings are static
traces. Only the bounded unknown own-frame writer/reader was executed; no
complete native archive round trip or Samsung appearance comparison is claimed.

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

## Native opaque records, wrappers and resources

At a layer root, `LayerDocLoadHandler::ReadUnknownObject_WDoc`, Model
`0x359038`, constructs current type 19 and applies the original outer type and
binary through `ObjectUnknown::NewApplyUnknownBinary` (`0x359100`–`0x35912c`).
The [type-100 admission finding](brush-record-findings.md#type-15-is-a-compatibility-dispatch-envelope)
uses this route. Successful admission first requires a valid common base.
It then allocates and copies the **entire supplied original object binary**,
including that common base (`0x44fc48`–`0x44fc6c`), and separately stores original
type, binary length and original load format version (`0x44fc74`–`0x44fc88`).
This native ownership is stronger than the Rust physical index's external
byte-range reference.

The ordinary WDoc save path writes the object's current type, from `GetType()`
at Model `0x355460`, as the outer byte at `0x355490`. Unknown construction sets
that current type to 19 (`0x44f198`–`0x44f1a0`). `ObjectUnknown::NewGetBinary`,
`0x44f89c`, writes reconstructed current common data followed by an unknown own
frame; it does not return the original record wholesale. For a non-null original
binary without the optional string, `ObjectUnknownImpl::GetOwnBinary`,
`0x450984`, emits own-frame kind 19, original type/version, a four-byte original
length and the unchanged original bytes. For an original length `N`, the frame
occupies `N + 27` bytes; its flexible-data mask sets bit 1
(`0x4505bc`–`0x4505d8`).
Current outer type, own-frame kind, original outer type and original load version
therefore remain separate identities. Original byte preservation does not make
the reframed outer record byte-identical to its input.

A Rust harness using Unicorn's C API executed the actual native own-frame size,
writer and reader (`0x45093c`, `0x450984`, `0x450aec`), including native mask
parsing and bounded-copy logic. Four synthetic cases used original lengths
0, 1, 8 and 257; types 100, 27 and 255; and versions 7, 17 and `u32::MAX`.
Across allocation fills `00`, `a5` and `ff`, the wrappers and reloaded original
bytes/type/version were identical. Allocation, deallocation, memcpy and logging
were host boundaries. This proves the selected no-string own-frame kernel,
not complete original-record admission, resource reachability or archive saving.
The temporary capture SHA-256 is
`b1682308302f9807c7524e6d6c11e8a6e39fcc4b726574561b86aa15a73b6d6e`.

Opaque admission also depends on location. Layer-root out-of-mask records use
the unknown reader at Model `0x358728`–`0x358740` and join ordinary layer
insertion. `ReadObjectContainer_WDoc`, `0x358a74`, instead sends out-of-mask,
non-type-4 **children** to `SkipDefaultObject` at `0x358c24`–`0x358c48`.
Known children are appended, and type-4 containers recurse. Already wrapped
current type 19 is admitted by the known-type mask `0x01cfe58f`; an original
future-type child such as type 100 is a different case. This APK's skip branch
does not establish that the format forbids opaque children.

Media preservation has a separate gate. `MediaFileManagerNew::Bind(int)` and
`Release(int)` change metadata offset 24, the live binding count. `saveItem`,
Model `0x28fe5c`, admits nonzero-count resources after validation; zero-count
resources require the manager's coedit flag to be **true** and the operation's
fourth bool to be **false** (`0x28feb0`–`0x28fec8`). WDoc's `WNote::IsCoeditMode`,
`0x96a90`, reads WNoteImpl offset 854; `InitSubComponent` passes that same flag to the manager
(`0xa3844`–`0xa384c`, Model `0x28c2cc`). It is not an unknown-object flag.
For admitted entries, `saveItem` writes the existing numeric media ID and
filename (`0x28ff14`–`0x28ff58`), without rewriting IDs inside opaque binaries.
Its map-key field is copied, although the broader save path can refresh PDF
hashes before this iteration (`0x290458`–`0x29045c`).

`RemoveUnusedFiles`, Model `0x292cec`, additionally protects zero-count files
whose filename and ID match the **previously saved** manifest
(`0x293464`–`0x29348c`). This deletion protection does not establish inclusion
in the next archive. Unknown's resolved original-version callback only lowers
the note's minimum unknown version (WDoc `0xa7bb8`–`0xa7bd0`); it does not bind
resources. The inspected unknown own-data paths contain no resource-ID scanner.
Decoded common image data can still register through `ObjectBase::OnAttach`
(Model `0x2d0018`). Owning opaque bytes and keeping admitted IDs stable do not
establish preservation of every attachment referenced only by opaque own data.

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
