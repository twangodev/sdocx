# PDF paper resource resolution

These findings concern Samsung Notes APK 4.4.45.37. Java paths are relative to
the decompiled `sources/` directory; addresses are virtual addresses in the
APK's arm64 libraries. Resolution, availability and PDF opening below are
static findings. No executed capture here establishes password handling,
coedit download behavior, PDF drawing parity or vector preservation.

The saved page-list encoding and versioned rectangle representation are
described in [page-background-findings.md](page-background-findings.md), with
native load filtering and record lifecycle in
[pdf-paper-storage-findings.md](pdf-paper-storage-findings.md).
The media manifest encoding and existing image resolver are described in
[image-findings.md](image-findings.md).

## Binding IDs, file hashes and filenames

A binary page PDF record references a signed media binding ID. It does not
contain an archive filename, filesystem path, URL or password. The PDF page
index is a separate field; it does not identify the media file.

`n1/a.java:49–72` reads a media manifest record into these separate members:

| Member | Meaning | Resolution role |
| --- | --- | --- |
| `f4066a` | Numeric binding ID | Binary page and object reference |
| `f4067f` | Filename | Relative to the document's `media/` directory |
| `k` | File hash | Content identity used by the synchronization mapping |
| `e` | Hash calculated from record metadata | Different from the asset's file hash |
| `c` | Absolute extracted path | Constructed from cache directory, `media/`, filename |
| `l` | Attached flag | Retained metadata; not a successful-open result |

The reader constructs `e` by hashing a concatenation of the file hash, binding
ID, filename, reference count and modification time, plus the attached flag
for the modern record. It constructs `c` as `b + "/media/" + f4067f`.
`n1/a.java:36–46` writes both `hash` and `fileHash` in synchronization XML;
these names must not be treated as interchangeable asset digests.

`m1/b.java:43–52` installs mappings by both binding ID and file hash.
`d(String)` at lines 81–93 maps a file hash to a binding ID and throws a
synchronization exception when the hash is unknown. `e(int)` at lines 95–103
maps a binding ID to its file hash and rejects ID `-1` or an absent mapping.

The page synchronization writer at `n1/u.java:237–248` uses this binding-to-file-
hash mapping for the `pdfData.hash` attribute. Its reader at lines 719–740
maps that attribute back through `m1/b.d`. The XML PDF reference is therefore
a file hash; the ordinary binary PDF reference is a numeric binding ID. Neither
is obtained by parsing a numeric filename prefix or by choosing the first PDF
in an archive.

The Java content record also exposes URL and upload-token state
(`n1/a.java:36–46,105–107`), but those values are not written by its binary
manifest writer at lines 71–103. Their presence in synchronization state does
not establish a URL field in the ordinary page PDF record.

## Runtime path construction

WDoc `WNote::GetPDFFilePathById` at `0x96af0` delegates through the document's
media manager vtable slot `+0x50`. For the new manager this resolves to Model
`MediaFileManagerNew::GetFilePathById` at `0x28f98c`; the vtable address point
is `0x491a40` and the relocation is at `0x491a90`.

That implementation searches metadata entries for the requested binding ID
(`0x28f9c4–0x28fa14`). A match constructs the output from the internal media
directory, a slash, and the matched filename (`0x28fa18–0x28fa44`).
`MediaFileManagerNew::Construct` at `0x28c160` establishes the media directory
by appending `/media` to its supplied internal directory (`0x28c254–0x28c278`;
constant at `0x1545fb`). Thus the runtime path is:

```text
<document internal directory>/media/<manifest filename>
```

The getter's successful return means that a binding was found. It does not
perform a filesystem-existence check, hash verification or PDF decoding.

WDoc `WPageImpl::m_UpdatePDFData(MediaFileManager*)` at `0xcf9ac` refreshes
each record's runtime path using that same slot. An unresolved ordinary
binding clears the path and changes its binding ID to `-1`
(`0xcfa00–0xcfa30`). Binding `-1000` bypasses this refresh; it is an external
PDF sentinel, not an unsigned archive resource ID. The separate
`WPageImpl::ReferExternalPDF` path at `0xceb10` releases an ordinary bound
resource, clears the path and installs that sentinel with external-reference
state. These runtime transitions do not establish that a saved file contains
an external resource or that it can be downloaded.

`SpenWPage.PDFData` at
`com/samsung/android/sdk/pen/worddoc/SpenWPage.java:458–471` exposes a read-only
binding ID and file path alongside mutable index, `RectF` and password fields.
WDoc `JNI_PDFDataList::GetJPDFData` at `0xe1dd0` populates them from the native
record: runtime path at `+0`, binding ID at `+0x10`, page index at `+0x14`,
rectangle at `+0x18`, and password string at `+0x28`.
The path and password are runtime record members, not extra fields beside the
saved page rectangle.

## Replacing source bytes and retargeting placements

Two native operations have different identity contracts. WDoc
`WNote::UpdatePDFFile`, `0x9e87c`, delegates to the page manager. It selects
pages whose PDF records match the old binding ID, then calls Model `BindPDF`
once for the replacement path (`0xb828c`/`0xb82d0`). With no matching pages it
returns true without binding that source. `BindPDF`, Model `0x28d964`, can
allocate a new ID or reuse a content-hash match through `Bind(path)`
(`0x28daf0`, `0x28c760`–`0x28c79c`); its returned ID is not necessarily new.

WPageImpl's update changes matching records' IDs at `0xcea78`, then releases
the old binding, binds the returned ID and resolves its path
(`0xcea90`/`0xceaa4`/`0xceabc`). Those per-record return values are ignored.
The direct body retains list order/count, source page indices, rectangles and
passwords; it does not recreate the PDF list, query the replacement's page
count or recompute placements. A retained index is not proof that the new PDF
contains that page. History and outer errors do not establish a transactional
update. The [record lifecycle](pdf-paper-storage-findings.md#runtime-pdf-data-is-richer-than-the-saved-record)
remains separate from this binding change.

`WNote::ReplacePDFFile`, `0x9e93c`, instead calls Model `ReplacePDF` at
`0x9e9a8`; it does not invoke that page-update route. Its supplied bool is
unused in this facade, which returns true for a nonnegative Model result.
Model `0x28e1d4` checks a supplied path exists and finds metadata by numeric
ID. On its no-conflict successful path, it retains the metadata pointer,
numeric ID, managed filename and live binding count while installing supplied
managed bytes, rekeying the map from the supplied content hash and recording
the target size
(`0x28e6a4`–`0x28e730`). BindFile checks copy/rename success for distinct paths;
this does not compare original and replacement bytes or validate PDF content.

Direct replacement is not an atomic swap. It detaches metadata, erases the old
map entry and unlinks the existing managed file (`0x28e474`–`0x28e480`)
before hashing or installing the new source. Later unlink/hash/copy failure
can return failure without a visible old-file/map rollback in this body. An
equal hash already in the map takes the conflict/error/`ForcedFC` route
(`0x28e5a0`–`0x28e630`), rather than successful deduplication to another ID.
This is static failure ordering, not an executed replacement loss.

There is a concrete source-rewrite caller. Composer JNI
`Native_updateAttachedFile`, `0x3193c8`, reaches
`NotePDFManager::UpdateAttachedFile`, `0x36c390`. After gathering existing PDF
bindings it checks `HasChangedFormField`, then calls RebindPdf only if true
(`0x36c40c`/`0x36c418`). RebindPdf generates a temporary path, checks extractor
SaveFile with `(temp,true,false,false,true)`, and checks
`ReplacePDFFile(oldId,temp,true)` (`0x371424`/`0x37144c`/`0x371464`). The
selected Pdfium vtable resolves those slots to `IsFormDirty`, PDF `0xa5cb0`,
and `SaveFile`, `0xa50f8`; SaveFile returns true only for engine status zero.
This serializes extractor state before replacing content behind the old ID;
it does not select the placement-retargeting operation above.

Java `SpenNotePdfManager.java:1251–1256` exposes the native bool.
`PdfManager.java:551–552` implements a void PDFDataSource callback that only
logs it, registered by `ModelManager.java:114`.
`NotesDocument.java:612–615` calls that callback immediately before
`saveCacheDocument()` in writing mode **100 (PDF Writing)**. A returned false
native result is not propagated through this void callback to that cache save.
This is a specific branch, not every ordinary note save; the inspected
loop does not undo earlier successful replacements when a later one fails.

The public four-argument media Save overload sets its private trailing flag
false (`0x290f04`), reaching `RefreshAllPdfFileHash` before saving entries
(`0x29045c`). Refresh processes map keys beginning `pdf_`, including placeholder
keys allocated by BindPDF, and hashes current managed root/filename
(`0x28dfc0`/`0x28dfcc`). It does not restore original imported bytes or change
page indices/placements. Moving metadata to an absent new hash key retains
its ID, but equal-hash collision has a separate static boundary: unique-map
insertion returns without replacing an existing key (`0x2983d0`–`0x2983f8`),
then refresh erases its old node without checking insertion success
(`0x28e094`/`0x28e0b0`). Unconditional ID retention across collisions is not
established; no collision execution is claimed here.

For entries admitted by the existing
[media save gates](vector-retention-findings.md#native-opaque-records-wrappers-and-resources),
saveItem writes the current ID, filename and map key, and its checked archive
add selects that managed filename (`0x290078`–`0x290084`). It does not select an
original import backup or the updater's temporary path. A stable binding can
therefore name changed PDF bytes. For Rust source retention, binding identity,
current managed content and retained original source bytes are separate
facts. These source-only calls do not establish a complete replacement/archive
round trip or the [vector transport guarantees](pdf-paper-export-findings.md)
of either exporter.

## Presence, availability and successful opening

These are three different native questions:

| Question | Entry point | Established behavior |
| --- | --- | --- |
| Does the page have PDF data? | WDoc `WPage::HasPDF`, `0xc6228` | Tests whether the page PDF list is nonempty |
| Is a bound PDF file available? | WDoc `WPage::IsPDFAvailable`, `0xcad7c` | Resolves the binding and checks file existence |
| Can this PDF be opened? | PDF `PdfiumImpl::Open`, `0x85184` | Calls PDFium with path and password and handles an open error |

`IsPDFAvailable` dispatches through media-manager slot `+0xb8`. Both old and
new manager vtables resolve this to Model
`MediaFileManager::CheckBoundFileExist(int)` at `0x28b504`; the new-manager
relocation is `0x491af8`. It calls the virtual path getter at `0x28b550`, then
`File::IsExist` at `0x28b55c`. No PDF header, page count, asset digest or
password check occurs in this predicate.

Composer `PageBackgroundView::loadPDF` at `0x3edb2c` checks intersecting PDF
views. At `0x3edbcc` it queries `IsPDFAvailable`; availability releases the
coedit synchronization view and calls `PDFView::Load` at `0x3edbe4`.
Unavailability calls `makeCoeditSyncView` at `0x3edbf0`. This establishes an
explicit unavailable-content presentation path. It does not establish that
every missing asset is remotely retrievable or that the placeholder repairs
the document by itself.

Composer's capture path is separate. `NotePDFCapture::DrawPDF` at `0x331ec4`
reads the ordered page PDF list. `getBitmap` at `0x3321e0` calls
`PdfRenderer::OpenFile(path,password)` at `0x332240`, then
`PdfRenderer::GetPage(index)` at `0x332270`. Failed open or an absent requested
page returns no bitmap; the caller skips that placement at `0x331fec`.
The capture path uses a bitmap and therefore does not prove native vector
PDF-page import for SDK exports.

PDF `PdfiumImpl::Open` converts the filename and passes the password to
`FPDF_LoadDocument` at `0x85220`. A successful open obtains the PDF page count
at `0x8523c`. A failed open calls `FPDF_GetLastError` at `0x85380` and maps
values 0–4 using the five-word table at `0x3ab68`: `[0,9,1,3,2]`; values outside
that range become 9. These map to success, unknown, I/O, format and password
results in `ISpenNotePdfManager.java:182–191`. The Java API also distinguishes
memory, out-of-range and protected results, without proving that this specific
file-open path emits them.

There is a separate outer-document encryption boundary:
`SpenWNote.java:1424–1446` checks the WDoc end tag, rejects an empty password
for an encrypted locked document, and calls `decryptWdoc` before construction.
`decryptWdoc` at lines 1346–1361 uses `WLockUtil.decrypt` to create a temporary
document. This is separate from PDFium's password for the embedded PDF. The
same string value or password storage policy cannot be inferred from the two
APIs.

## Consequences for the current Rust model

The existing Rust `MediaResolver` in `media.rs` already models authoritative
manifest ID-to-filename binding, including missing entries and ambiguity. Its
normal parsing does not verify the retained asset hash. Those contracts apply
at the archive-resource level; they do not certify a resolved file as a
supported or openable PDF.

`container.rs::parse_media_assets` currently includes only PNG, JPEG and WebP.
PDF ZIP entries can be present in the manifest and archive while absent from
`Document.media_assets`. This is an unsupported asset type in the current
extraction path, not evidence that the source archive has lost its PDF.

`page.rs` currently reads every PDF ID and rectangle but discards them, retaining
only the first page index as `PageTemplateSource::CustomPdf`. Consequently the
public model cannot distinguish multiple PDF placements, multiple bindings,
their rectangles, an unresolved resource, or a present PDF with an invalid
page index. The saved list's order and cardinality are independent of the
number of physical note pages.

The native boundaries distinguish archive binding, byte availability, PDF
opening, page selection and placement. Treating all five as a template type
or as an image asset index loses information. In particular, a PDF's own page
coordinates and its saved note placement rectangle describe different spaces;
the reference index and rectangle both belong to the placement.

Resource decoding and bitmap capture do not certify SVG conversion or the
codebase's vector-export guarantees. Samsung's separate export path reaches
PDF page import, as traced in
[pdf-paper-export-findings.md](pdf-paper-export-findings.md). File identity,
placement and export strategy remain separate contracts even when they use
the same source PDF.

## Binary provenance

| ARM64 library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libSPenPdf.so` | `cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
